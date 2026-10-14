// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! Service self-registration data for Bill8:Reporting.

use sentin8::registration::{
    AdminScope, AdminUi, AdminUiPage, ErrorCode, M2mGrant, PgRole, PolicySetDecl, Registration,
    SqlMigration,
};
use utoipa::OpenApi;

const PG_ROLES: &[PgRole] = &[PgRole {
    scope: "bill8:reporting:read,bill8:reporting:write",
    role: "bill8_reporting_service",
    bypass_rls: false,
    database: "bill8",
    cross_schema_grants: &[],
}];

const M2M_GRANTS: &[M2mGrant] = &[
    M2mGrant {
        workload: "bill8:reporting",
        scopes: &[
            "sentin8:bus:produce",
            "sentin8:bus:consume",
            "bill8:reporting:admin",
            "sentin8:stat:measure",
        ],
    },
    M2mGrant {
        workload: "reporting",
        scopes: &[
            "sentin8:bus:produce",
            "sentin8:bus:consume",
            "bill8:reporting:admin",
            "sentin8:stat:measure",
        ],
    },
    M2mGrant {
        workload: "reporting-audit",
        scopes: &["sentin8:sys:audit:write"],
    },
    M2mGrant {
        workload: "bill8:reporting-policy",
        scopes: &["sentin8:bus:consume", "sentin8:stat:measure"],
    },
    M2mGrant {
        workload: "reporting-policy",
        scopes: &["sentin8:stat:measure"],
    },
    M2mGrant {
        workload: "bill8-reporting-tls",
        scopes: &["sentin8:sys:instances:read"],
    },
];

const ADMIN_SCOPES: &[AdminScope] = &[
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:types:read",
    },
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:types:write",
    },
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:runs:read",
    },
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:runs:write",
    },
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:policy:read",
    },
    AdminScope::Typed {
        role: "bill8:reporting:admin",
        scope: "bill8:reporting:policy:write",
    },
];

const ERROR_CODES: &[ErrorCode] = &[
    ErrorCode {
        enum_name: "ReportingRunNotFound",
        error_code: "reporting_run_not_found",
        status_code: 404,
    },
    ErrorCode {
        enum_name: "ReportingTypeNotFound",
        error_code: "reporting_type_not_found",
        status_code: 404,
    },
];

const ADMIN_UI_PAGES: &[AdminUiPage] = &[
    AdminUiPage {
        route: "overview",
        label: "Overview",
        ui_endpoint: "/v1/admin/ui/overview",
        data_endpoint: "/v1/admin/overview",
        icon: "0xe1b1",
    },
    AdminUiPage {
        route: "report-types",
        label: "Report Types",
        ui_endpoint: "/v1/admin/ui/report-types",
        data_endpoint: "/v1/admin/report-types",
        icon: "0xe873",
    },
    AdminUiPage {
        route: "report-runs",
        label: "Report Runs",
        ui_endpoint: "/v1/admin/ui/report-runs",
        data_endpoint: "/v1/admin/report-runs",
        icon: "0xf0c6",
    },
];

/// The scope-mirror Shield text.
pub const SCOPE_MIRROR_POLICIES: &str = "permit(principal, action == Action::\"bill8:reporting:types:read\", resource) when { principal.scopes.contains(\"bill8:reporting:types:read\") };\npermit(principal, action == Action::\"bill8:reporting:types:read\", resource) when { principal.scopes.contains(\"sentin8:sys:admin\") };\npermit(principal, action == Action::\"bill8:reporting:types:write\", resource) when { principal.scopes.contains(\"bill8:reporting:types:write\") };\npermit(principal, action == Action::\"bill8:reporting:types:write\", resource) when { principal.scopes.contains(\"sentin8:sys:admin\") };\npermit(principal, action == Action::\"bill8:reporting:runs:read\", resource) when { principal.scopes.contains(\"bill8:reporting:runs:read\") };\npermit(principal, action == Action::\"bill8:reporting:runs:read\", resource) when { principal.scopes.contains(\"sentin8:sys:admin\") };\npermit(principal, action == Action::\"bill8:reporting:runs:write\", resource) when { principal.scopes.contains(\"bill8:reporting:runs:write\") };\npermit(principal, action == Action::\"bill8:reporting:runs:write\", resource) when { principal.scopes.contains(\"sentin8:sys:admin\") };\n";

const POLICIES: &[PolicySetDecl] = &[PolicySetDecl {
    policy_set_name: "scope-mirror",
    description: "Action-granular scope permits for bill8:reporting over the LIVE effective scope set. Step 5 is the only gate.",
    policies: SCOPE_MIRROR_POLICIES,
    schema: None,
    mode: "enforced",
    default_enabled: true,
}];

const SQL_1_INIT: &str = include_str!("../sql/init.sql");

fn openapi_spec() -> serde_json::Value {
    serde_json::to_value(crate::openapi::ReportingOpenApi::openapi())
        .unwrap_or(serde_json::Value::Null)
}

/// Print the registration JSON to stdout.
pub fn print(section: Option<String>) -> anyhow::Result<()> {
    let reg = Registration {
        service_type: "bill8:reporting",
        subdomain: "bill8-reporting",
        health_path: "/v1/health",
        admin_scopes: ADMIN_SCOPES,
        error_codes: ERROR_CODES,
        consent_providers: &[],
        consent_flows: &[],
        policies: POLICIES,
        relations: &[],
        event_triggers: &[],
        schedules: &[],
        admin_ui: Some(AdminUi {
            label: "Reporting",
            icon: "0xe873",
            service_flag: "reporting",
            pages: ADMIN_UI_PAGES,
        }),
        sql_migrations: vec![SqlMigration {
            version: 1,
            name: "init",
            sql: SQL_1_INIT,
        }],
        pg_roles: PG_ROLES,
        audit_findings: None,
        service_actions: None,
        m2m_grants: Some(M2M_GRANTS),
        openapi_url: "/openapi.json",
        openapi: openapi_spec,
    };
    sentin8::registration::print_registration(section, &reg, openapi_spec)
}
