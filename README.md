# Bill8:Reporting - Reports

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Docs](https://img.shields.io/badge/docs/read-3aa757.svg)](https://bill8.dev/docs/services/reporting)
[![Website](https://img.shields.io/badge/website-bill8.dev-1a73e8.svg)](https://bill8.dev)

Report types, report runs, and scheduled reports.

## 1. Summary

Bill8:Reporting owns the reporting surface: report types that define
a query shape, report runs that execute them, and scheduled reports that
deliver output files via Storage.

## 2. Motivation

Merchants need period-end and operational reports without building them.
Centralizing report definitions and runs keeps reporting concerns out of
domain services and gives operators a consistent output contract.

## 3. Architecture & High-Level Design

```mermaid
graph TD
    BFF["Sentin8:BFF /proxy/reporting/*"] --> R["Bill8:Reporting"]
    CRON["Sentin8:Cron"] -->|"scheduled runs"| R
    R -->|"output files"| STOR["Sentin8:Storage"]
    R -->|"reporting.outbox → relay"| BUS["Sentin8:Bus"]
    R <--> PG[("bill8 database<br/>schema reporting")]
```

## 4. Key Traits and Data Structures

- `ReportType` - a named report definition with a versioned query shape.
- `ReportRun` - an execution of a type with a status
  (`pending` → `succeeded` / `failed`) and an output reference.

## 5. Rationale, Trade-offs, and Alternatives

- **Report runs over live queries**: a run snapshots data at a point in
  time, so a period-end report is reproducible.
- **Delegating scheduling to Sentin8:Cron** over an embedded scheduler.

## 6. Internal Implementation & Invariants

- A run is idempotent per `(env_id, report_type, period)`.
- Output files are written to Storage and referenced, never inlined.

## 7. Unresolved Questions / Future Designs

- See `DESIGN.md` §10.

---

**Architecture & Design:** bill8:reporting and its underlying concepts were designed by [Hamid Jahanzad](https://www.linkedin.com/in/hamidjahanzad/).
