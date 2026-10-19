// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! Reporting HTTP handlers - report types and runs. A run is idempotent
//! per `(env_id, report_type, period)`; output files are Storage
//! references, never inline.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use bill8::reporting::contracts::{CreateReportRunRequest, ReportRun, ReportType};
use sqlx::Row;
use bill8::wire::contracts::HealthBuild;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth;
use crate::config::ReportingConfig;
use crate::policy::Engine;

pub mod schedules;
pub use schedules::{
    create_report_schedule, delete_report_schedule, list_report_schedules, patch_report_schedule,
    run_report_schedules,
};

/// Shared application state bound into the shell.
#[derive(Clone)]
pub struct AppState {
    /// Service configuration.
    pub config: Arc<ReportingConfig>,
    /// Capped PostgreSQL pool.
    pub pool: sqlx::PgPool,
    /// M2M-backed Bus client.
    pub bus_client: sentin8::bus::client::BusClient,
    /// Shield policy engine - step 5 is the ONLY gate.
    pub policy: Arc<Engine>,
}

/// Set the transaction-local RLS tenant context.
pub async fn set_tenant_context(
    conn: &mut sqlx::PgConnection,
    env_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT sentin8_set_tenant_context($1, NULL, NULL, NULL, false, false, false, NULL, 'reporting')")
        .bind(env_id)
        .execute(conn)
        .await
        .map(|_| ())
}

/// Standard DB error → HTTP 500.
pub fn err_db(e: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[utoipa::path(
    get,
    path = "/v1/health",
    responses(
        (status = 200, description = "Liveness (status, service, version, build)", body = bill8::wire::contracts::HealthBuild),
    ),
    security(),
    tag = "meta",
)]
/// Health probe payload.
pub async fn health() -> axum::Json<HealthBuild> {
    axum::Json(HealthBuild {
        status: "ok".to_string(),
        service: "reporting".to_string(),
        version: sentin8::build_info::id(),
        build: sentin8::build_info::build_hash().to_string(),
    })
}

/// `POST /v1/report-types` - create a report type.
#[utoipa::path(
    post,
    path = "/v1/report-types",
    responses(
        (status = 200, description = "Report type created", body = ReportType),
        (status = 403, description = "Missing bill8:reporting:types:write"),
    ),
    security(("bill8:reporting:types:write" = [])),
    tag = "reporting",
)]
pub async fn create_report_type(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ReportType>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:types:write"),
    )
    .await?;
    let name = body["name"].as_str().unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required".into()));
    }

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;
    let id = Uuid::new_v4();
    let row = sqlx::query(
        r#"INSERT INTO reporting.report_types (id, env_id, name)
           VALUES ($1, $2, $3)
           RETURNING id, env_id, name, version, created_at"#,
    )
    .bind(id)
    .bind(a.env_id())
    .bind(&name)
    .fetch_one(&mut *conn)
    .await
    .map_err(err_db)?;
    conn.commit().await.map_err(err_db)?;

    Ok(Json(ReportType {
        id: row.get("id"),
        env_id: row.get("env_id"),
        name: row.get("name"),
        version: row.get("version"),
        created_at: row.get("created_at"),
    }))
}

/// `GET /v1/report-types` - list report types.
#[utoipa::path(
    get,
    path = "/v1/report-types",
    responses(
        (status = 200, description = "List of report types", body = [ReportType]),
        (status = 403, description = "Missing bill8:reporting:types:read"),
    ),
    security(("bill8:reporting:types:read" = [])),
    tag = "reporting",
)]
pub async fn list_report_types(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ReportType>>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:types:read"),
    )
    .await?;

    let mut conn = state.pool.acquire().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let rows = sqlx::query(
        "SELECT id, env_id, name, version, created_at FROM reporting.report_types ORDER BY created_at",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(err_db)?;

    Ok(Json(
        rows.iter()
            .map(|r| ReportType {
                id: r.get("id"),
                env_id: r.get("env_id"),
                name: r.get("name"),
                version: r.get("version"),
                created_at: r.get("created_at"),
            })
            .collect(),
    ))
}

/// `POST /v1/report-runs` - run a report (idempotent per
/// `(env_id, type, period)`; two identical runs yield one row).
#[utoipa::path(
    post,
    path = "/v1/report-runs",
    request_body = CreateReportRunRequest,
    responses(
        (status = 200, description = "Report run created", body = ReportRun),
        (status = 403, description = "Missing bill8:reporting:runs:write"),
        (status = 409, description = "Run already exists for this period (idempotent)"),
    ),
    security(("bill8:reporting:runs:write" = [])),
    tag = "reporting",
)]
pub async fn create_report_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateReportRunRequest>,
) -> Result<Json<ReportRun>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:runs:write"),
    )
    .await?;

    let mut conn = state.pool.begin().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    // Idempotent per (env, type, period): return the existing run.
    if let Ok(Some(row)) = sqlx::query(
        "SELECT id, env_id, report_type_id, period_start, period_end, status, output_ref, created_at \
         FROM reporting.report_runs WHERE env_id = $1 AND report_type_id = $2 AND period_start = $3 AND period_end = $4",
    )
    .bind(a.env_id())
    .bind(req.report_type_id)
    .bind(req.period_start)
    .bind(req.period_end)
    .fetch_optional(&mut *conn)
    .await
    {
        conn.commit().await.map_err(err_db)?;
        return Ok(Json(row_to_run(&row)));
    }

    let id = Uuid::new_v4();
    let row = sqlx::query(
        r#"INSERT INTO reporting.report_runs
               (id, env_id, report_type_id, period_start, period_end, status, idempotency_key)
           VALUES ($1, $2, $3, $4, $5, 'pending', $6)
           RETURNING id, env_id, report_type_id, period_start, period_end, status, output_ref, created_at"#,
    )
    .bind(id)
    .bind(a.env_id())
    .bind(req.report_type_id)
    .bind(req.period_start)
    .bind(req.period_end)
    .bind(req.idempotency_key.as_deref())
    .fetch_one(&mut *conn)
    .await
    .map_err(err_db)?;

    sqlx::query(
        r#"INSERT INTO reporting.outbox (env_id, event_type, payload)
           VALUES ($1, 'bill8:reporting:report_run:created', $2)"#,
    )
    .bind(a.env_id())
    .bind(serde_json::json!({ "report_run_id": id }))
    .execute(&mut *conn)
    .await
    .map_err(err_db)?;

    conn.commit().await.map_err(err_db)?;
    Ok(Json(row_to_run(&row)))
}

/// `GET /v1/report-runs` - list runs.
#[utoipa::path(
    get,
    path = "/v1/report-runs",
    responses(
        (status = 200, description = "List of runs", body = [ReportRun]),
        (status = 403, description = "Missing bill8:reporting:runs:read"),
    ),
    security(("bill8:reporting:runs:read" = [])),
    tag = "reporting",
)]
pub async fn list_report_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ReportRun>>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:runs:read"),
    )
    .await?;

    let mut conn = state.pool.acquire().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let rows = sqlx::query(
        "SELECT id, env_id, report_type_id, period_start, period_end, status, output_ref, created_at FROM reporting.report_runs ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(err_db)?;

    Ok(Json(rows.iter().map(row_to_run).collect()))
}

/// `GET /v1/report-runs/{id}` - read one run.
#[utoipa::path(
    get,
    path = "/v1/report-runs/{id}",
    responses(
        (status = 200, description = "The run", body = ReportRun),
        (status = 404, description = "Run not found"),
    ),
    security(("bill8:reporting:runs:read" = [])),
    tag = "reporting",
)]
pub async fn get_report_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ReportRun>, (StatusCode, String)> {
    let a = auth::verify_token(&state.config, &headers).await?;
    crate::policy::require(
        &state.policy,
        &a,
        &headers,
        sentin8::shield::EvalTarget::new("bill8:reporting:runs:read"),
    )
    .await?;

    let mut conn = state.pool.acquire().await.map_err(err_db)?;
    set_tenant_context(&mut conn, a.env_id())
        .await
        .map_err(err_db)?;

    let row = sqlx::query(
        "SELECT id, env_id, report_type_id, period_start, period_end, status, output_ref, created_at FROM reporting.report_runs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(err_db)?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "report run not found".into()))?;

    Ok(Json(row_to_run(&row)))
}

fn row_to_run(row: &sqlx::postgres::PgRow) -> ReportRun {
    ReportRun {
        id: row.get("id"),
        env_id: row.get("env_id"),
        report_type_id: row.get("report_type_id"),
        period_start: row.get("period_start"),
        period_end: row.get("period_end"),
        status: row.get("status"),
        output_ref: row.get("output_ref"),
        created_at: row.get("created_at"),
    }
}
