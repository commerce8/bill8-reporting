// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! OpenAPI spec for the Bill8:Reporting API.

use utoipa::OpenApi;

use crate::handlers;

/// The reporting service's OpenAPI document.
#[derive(OpenApi)]
#[openapi(paths(
    handlers::health,
    handlers::create_report_type,
    handlers::list_report_types,
    handlers::create_report_run,
    handlers::list_report_runs,
    handlers::get_report_run,
    handlers::schedules::create_report_schedule,
    handlers::schedules::list_report_schedules,
    handlers::schedules::patch_report_schedule,
    handlers::schedules::delete_report_schedule,
    handlers::schedules::run_report_schedules,
))]
pub struct ReportingOpenApi;
