// SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
// SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
// SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
// SPDX-License-Identifier: Apache-2.0

//! # Bill8:Reporting — reports
//!
//! Report types and runs. A run is idempotent per `(env_id, report_type,
//! period)`; output files are Storage references, never inline. See
//! `DESIGN.md`.

#![forbid(unsafe_code)]

pub mod auth;
pub mod config;
pub mod handlers;
pub mod openapi;
pub mod policy;
pub mod registration;
pub mod server;
