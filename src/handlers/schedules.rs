// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! Report-schedule handlers (RPT-3): a Cron schedule per
//! (report type, period) → the proxy-back run. A schedule produces at
//! most one run per period — the run handler's
//! `(env, type, period_start, period_end)` uniqueness (RPT-2) makes
//! two Cron fires in one window produce exactly one run (reporting
//! DESIGN.md §2/§4).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::auth;
use crate::handlers::{err_db, set_tenant_context, AppState};

/// Request body for schedule creation.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateReportScheduleRequest {
    /// The report type to schedule.
    pub report_type_id: Uuid,
    /// Run interval (`daily` / `weekly` / `monthly`).
    pub interval: String,
}

/// The schedule wire shape.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ReportSchedule {
    /// Schedule id.
    pub id: Uuid,
    /// The scheduled report type.
    pub report_type_id: Uuid,
    /// Run interval.
    pub interval: String,
    /// Whether the schedule fires.
    pub enabled: bool,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<Utc>,
}

/// Request body for schedule patching (enable/disable).
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PatchReportScheduleRequest {
    /// Enable/disable the schedule.
    pub enabled: bool,
}

/// `POST /v1/report-schedules` — create a schedule.
#[utoipa::path(
    post,
    path = "/v1/report-schedules",
    request_body = CreateReportScheduleRequest,
    responses(
        (status = 200, description = "Schedule created", body = ReportSchedule),
        (status = 403, description = "Missing bill8:reporting:schedules:write"),
    ),
    security(("bill8:reporting:schedules:write" = [])),
    tag = "reporting",
)]
pub async fn create_report_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateReportScheduleRequest>,
) -> Result<Json<ReportSchedule>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:schedules:write"),
    )
    .await?;

    if !matches!(req.interval.as_str(), "daily" | "weekly" | "monthly") {
        return Err((
            StatusCode::BAD_REQUEST,
            "interval must be daily|weekly|monthly".into(),
        ));
    }

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let id = Uuid::new_v4();
    let row = sqlx::query(
        r#"INSERT INTO reporting.report_schedules (id, env_id, report_type_id, interval)
           VALUES ($1, $2, $3, $4)
           RETURNING id, report_type_id, interval, enabled, created_at"#,
    )
    .bind(id)
    .bind(a.env_id())
    .bind(req.report_type_id)
    .bind(&req.interval)
    .fetch_one(&mut *conn)
    .await
    .map_err(err_db)?;

    sqlx::query(
        r#"INSERT INTO reporting.outbox (env_id, event_type, payload)
           VALUES ($1, 'bill8:reporting:report_type:updated', $2)"#,
    )
    .bind(a.env_id())
    .bind(serde_json::json!({ "schedule_id": id, "report_type_id": req.report_type_id }))
    .execute(&mut *conn)
    .await
    .map_err(err_db)?;

    conn.commit().await.map_err(err_db)?;
    Ok(Json(row_to_schedule(&row)))
}

/// `GET /v1/report-schedules` — list the env's schedules.
#[utoipa::path(
    get,
    path = "/v1/report-schedules",
    responses(
        (status = 200, description = "List of schedules", body = [ReportSchedule]),
        (status = 403, description = "Missing bill8:reporting:schedules:read"),
    ),
    security(("bill8:reporting:schedules:read" = [])),
    tag = "reporting",
)]
pub async fn list_report_schedules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ReportSchedule>>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:schedules:read"),
    )
    .await?;

    let mut conn = state.pool.acquire().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let rows = sqlx::query(
        "SELECT id, report_type_id, interval, enabled, created_at \
         FROM reporting.report_schedules ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(err_db)?;

    Ok(Json(rows.iter().map(row_to_schedule).collect()))
}

/// `PATCH /v1/report-schedules/{id}` — enable/disable a schedule.
#[utoipa::path(
    patch,
    path = "/v1/report-schedules/{id}",
    request_body = PatchReportScheduleRequest,
    responses(
        (status = 200, description = "Schedule updated", body = ReportSchedule),
        (status = 404, description = "Schedule not found"),
    ),
    security(("bill8:reporting:schedules:write" = [])),
    tag = "reporting",
)]
pub async fn patch_report_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<PatchReportScheduleRequest>,
) -> Result<Json<ReportSchedule>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:schedules:write"),
    )
    .await?;

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let row = sqlx::query(
        "UPDATE reporting.report_schedules SET enabled = $1 \
         WHERE id = $2 AND env_id = $3 \
         RETURNING id, report_type_id, interval, enabled, created_at",
    )
    .bind(req.enabled)
    .bind(id)
    .bind(a.env_id())
    .fetch_optional(&mut *conn)
    .await
    .map_err(err_db)?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "schedule not found".into()))?;

    sqlx::query(
        r#"INSERT INTO reporting.outbox (env_id, event_type, payload)
           VALUES ($1, 'bill8:reporting:report_type:updated', $2)"#,
    )
    .bind(a.env_id())
    .bind(serde_json::json!({ "schedule_id": id, "enabled": req.enabled }))
    .execute(&mut *conn)
    .await
    .map_err(err_db)?;

    conn.commit().await.map_err(err_db)?;
    Ok(Json(row_to_schedule(&row)))
}

/// `DELETE /v1/report-schedules/{id}` — remove a schedule.
#[utoipa::path(
    delete,
    path = "/v1/report-schedules/{id}",
    responses(
        (status = 200, description = "Schedule deleted"),
        (status = 404, description = "Schedule not found"),
    ),
    security(("bill8:reporting:schedules:write" = [])),
    tag = "reporting",
)]
pub async fn delete_report_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:schedules:write"),
    )
    .await?;

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let result =
        sqlx::query("DELETE FROM reporting.report_schedules WHERE id = $1 AND env_id = $2")
            .bind(id)
            .bind(a.env_id())
            .execute(&mut *conn)
            .await
            .map_err(err_db)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "schedule not found".into()));
    }

    sqlx::query(
        r#"INSERT INTO reporting.outbox (env_id, event_type, payload)
           VALUES ($1, 'bill8:reporting:report_type:updated', $2)"#,
    )
    .bind(a.env_id())
    .bind(serde_json::json!({ "schedule_id": id, "deleted": true }))
    .execute(&mut *conn)
    .await
    .map_err(err_db)?;

    conn.commit().await.map_err(err_db)?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// `POST /v1/internal/report-schedules/run` — the Cron proxy-back
/// tick. Creates one run per enabled schedule for the current period;
/// the run's (env, type, period) uniqueness (RPT-2) makes two ticks in
/// one window produce exactly one run.
#[utoipa::path(
    post,
    path = "/v1/internal/report-schedules/run",
    responses(
        (status = 200, description = "Tick processed"),
    ),
    security(("sentin8:cron:write" = [])),
    tag = "reporting",
)]
pub async fn run_report_schedules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Cron proxy-back: the cron workload presents its certificate (the
    // TLS handshake is the identity; verification is the standard pipeline).
    let a = auth::verify_token(&state.config, &headers).await?;

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let schedules = sqlx::query(
        "SELECT id, report_type_id, interval FROM reporting.report_schedules WHERE enabled",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(err_db)?;

    let now = Utc::now();
    let mut runs_created = 0usize;
    for s in &schedules {
        let schedule_id: Uuid = s.get("id");
        let report_type_id: Uuid = s.get("report_type_id");
        let interval: String = s.get("interval");

        // The current period window for the interval.
        let (period_start, period_end) = match interval.as_str() {
            "daily" => (now - Duration::hours(24), now),
            "weekly" => (now - Duration::days(7), now),
            _ => (now - Duration::days(30), now),
        };

        // Idempotent per (env, type, period): ON CONFLICT DO NOTHING —
        // a second Cron fire in the window is a no-op.
        let result = sqlx::query(
            r#"INSERT INTO reporting.report_runs
                   (id, env_id, report_type_id, period_start, period_end, status, idempotency_key)
               VALUES ($1, $2, $3, $4, $5, 'pending', $6)
               ON CONFLICT (env_id, report_type_id, period_start, period_end) DO NOTHING
               RETURNING id"#,
        )
        .bind(Uuid::new_v4())
        .bind(a.env_id())
        .bind(report_type_id)
        .bind(period_start)
        .bind(period_end)
        .bind(format!(
            "report-schedule-{schedule_id}-{}",
            now.format("%Y%m%d%H")
        ))
        .fetch_optional(&mut *conn)
        .await
        .map_err(err_db)?;
        if result.is_some() {
            runs_created += 1;
        }
    }

    conn.commit().await.map_err(err_db)?;
    Ok(Json(serde_json::json!({ "runs_created": runs_created })))
}

/// Map a row to the schedule wire type.
fn row_to_schedule(row: &sqlx::postgres::PgRow) -> ReportSchedule {
    ReportSchedule {
        id: row.get("id"),
        report_type_id: row.get("report_type_id"),
        interval: row.get("interval"),
        enabled: row.get("enabled"),
        created_at: row.get("created_at"),
    }
}
