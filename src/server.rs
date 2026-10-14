// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! Axum server setup for Bill8:Reporting — state binding + routes.

use axum::routing::get;
use std::sync::Arc;

use crate::handlers;

/// Build a cert-presenting HTTP transport for `workload` (CERTFREE-4V,
/// auth DESIGN §4.3f): mint-or-load the Acme-issued pair and wrap it
/// in a reqwest client. The receiving service's TLS layer authenticates
/// the handshake and the enroll-published registry map authorizes the
/// scopes — no bearer token exists on this path.
///
/// # Errors
/// Returns the certificate-ensure or transport-build failure (the
/// caller logs and degrades).
async fn build_workload_http(auth_issuer: &str, workload: &str) -> anyhow::Result<reqwest::Client> {
    let cert = sentin8::auth::m2m::ensure_workload_certificate(
        auth_issuer,
        workload,
        &sentin8::auth::m2m::platform_ca_path(),
    )
    .await?;
    let mtls = sentin8::middleware::mtls::MtlsConfig::new(
        &cert.ca_cert_path,
        &cert.cert_path,
        &cert.key_path,
    );
    sentin8::middleware::mtls::build_mtls_client(&mtls)
        .map_err(|e| anyhow::anyhow!("workload '{workload}' transport: {e}"))
}

/// Bind the service state.
pub async fn build_state(core: sentin8::shell::ServiceCore) -> anyhow::Result<handlers::AppState> {
    let mut config = crate::config::ReportingConfig::from_env();

    // Cert-presenting Bus client (CERTFREE-4V): the `reporting` workload
    // CERTIFICATE is the identity — no DCR, no token (auth DESIGN
    // §4.3f); the mint retries while the fleet boots.
    let bus_client = loop {
        match sentin8::bus::client::BusClient::for_workload(
            &config.sentin8_bus_url,
            &config.sentin8_auth_issuer,
            "reporting",
        )
        .await
        {
            Ok(c) => break c,
            Err(e) => {
                tracing::debug!(error = %e, "reporting bus client: certificate not ready yet");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    };

    // The service's own workload transport (CERTFREE-4V): the
    // `reporting` certificate presents in the TLS handshake — the
    // receiving service's TLS layer authenticates it and the registry
    // map authorizes the scopes.
    match build_workload_http(&config.sentin8_auth_issuer, "reporting").await {
        Ok(http) => config.m2m_http = Some(http),
        Err(e) => tracing::warn!(error = %e, "reporting workload transport unavailable — degraded"),
    }

    let policy_engine = core.admin.policy.clone();

    Ok(handlers::AppState {
        config: Arc::new(config),
        pool: core.pool.clone(),
        bus_client,
        policy: policy_engine,
    })
}

/// Mount the reporting routes onto the shell router (state-typed).
pub fn routes(router: axum::Router<handlers::AppState>) -> axum::Router<handlers::AppState> {
    router
        .route("/v1/health", get(handlers::health))
        .route(
            "/v1/report-types",
            get(handlers::list_report_types).post(handlers::create_report_type),
        )
        .route(
            "/v1/report-runs",
            get(handlers::list_report_runs).post(handlers::create_report_run),
        )
        .route("/v1/report-runs/{id}", get(handlers::get_report_run))
        // Report schedules (RPT-3): per-(type, period) Cron schedules.
        .route(
            "/v1/report-schedules",
            get(handlers::list_report_schedules).post(handlers::create_report_schedule),
        )
        .route(
            "/v1/report-schedules/{id}",
            axum::routing::patch(handlers::patch_report_schedule)
                .delete(handlers::delete_report_schedule),
        )
        // The Cron proxy-back tick (the platform scheduler calls it).
        .route(
            "/v1/internal/report-schedules/run",
            axum::routing::post(handlers::run_report_schedules),
        )
}
