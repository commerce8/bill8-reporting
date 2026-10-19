// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! Reporting configuration loaded from environment variables.

use std::env;

use sentin8::auth::client::{Client as AuthClient, ClientOptions};

/// Reporting configuration.
#[derive(Clone)]
pub struct ReportingConfig {
    /// Service port (assigned by Sentin8:Captain).
    pub port: u16,
    /// PostgreSQL DSN (role `bill8_reporting_service`).
    pub database_url: String,
    /// Auth issuer base URL.
    pub sentin8_auth_issuer: String,
    /// Auth JWKS URL.
    pub sentin8_auth_jwks_url: String,
    /// Bus base URL.
    pub sentin8_bus_url: String,
    /// Shared Auth client.
    pub sentin8_auth_client: AuthClient,
    /// Certificate-presenting transport for the service's own workload
    /// identity (CERTFREE-4V - no M2M token exists).
    pub m2m_http: Option<reqwest::Client>,
}

impl std::fmt::Debug for ReportingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReportingConfig")
            .field("port", &self.port)
            .field("database_url", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ReportingConfig {
    /// Load configuration from environment variables.
    ///
    /// # Panics
    /// Panics when `BILL8_REPORTING_PORT` is unset or invalid.
    pub fn from_env() -> Self {
        let cfg = sentin8::config::Sentin8Config::from_env();
        let issuer = env::var("SENTIN8_AUTH_ISSUER").unwrap_or_else(|_| cfg.sentin8_auth_issuer());
        Self {
            port: env::var("BILL8_REPORTING_PORT")
                .expect("BILL8_REPORTING_PORT must be set by captain")
                .parse()
                .expect("BILL8_REPORTING_PORT must be a valid u16"),
            database_url: env::var("BILL8_REPORTING_DATABASE_URL")
                .or_else(|_| env::var("DATABASE_URL"))
                .unwrap_or_else(|_| {
                    sentin8::config::Sentin8Config::default_database_url().to_string()
                }),
            sentin8_auth_issuer: issuer.clone(),
            sentin8_auth_jwks_url: env::var("SENTIN8_AUTH_JWKS_URL")
                .unwrap_or_else(|_| cfg.sentin8_auth_jwks_url()),
            sentin8_bus_url: env::var("SENTIN8_BUS_URL").unwrap_or_else(|_| cfg.sentin8_bus_url()),
            sentin8_auth_client: AuthClient::new(ClientOptions {
                base_url: issuer.clone(),
                ..Default::default()
            }),
            m2m_http: None,
        }
    }
}
