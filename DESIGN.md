# Bill8:Reporting — Detailed Design Document

## 1. Scope & Context

Bill8:Reporting owns: the reporting domain schema (domain tables +
the leaf schema `reporting` for outbox/audit journal), the report-type /
report-run REST API, the scheduled-report proxy-back handler for
Sentin8:Cron, and the `bill8:reporting:*` event family.

It does NOT own: the underlying data (reports read via public APIs of
the owning services), object storage (Sentin8:Storage holds output
files), or scheduling (Sentin8:Cron).

```mermaid
graph TD
    BFF["Sentin8:BFF /proxy/reporting/*"] --> R["Bill8:Reporting"]
    CRON["Sentin8:Cron"] -->|"scheduled runs"| R
    R -->|"public API reads"| PAY["Bill8:Payments"]
    R -->|"public API reads"| L["Bill8:Ledger"]
    R -->|"output files"| STOR["Sentin8:Storage"]
    R -->|"reporting.outbox → relay"| BUS["Sentin8:Bus"]
    R <-->|"schema reporting"| PG[("bill8 database")]
```

## 2. Runtime Architecture

Shell-booted; `build_state` adds config, Bus client, AppState.

**Report run lifecycle**:

```mermaid
sequenceDiagram
    participant C as Client (BFF)
    participant R as Bill8:Reporting
    participant PAY as Bill8:Payments
    participant STOR as Sentin8:Storage
    participant DB as PostgreSQL
    C->>R: POST /v1/report-runs (report_type, period)
    R->>R: Shield gate
    R->>DB: idempotency check (env, type, period)
    R->>DB: report run status=pending + outbox report_run:created
    R->>PAY: GET /v1/charges?period (public API)
    R->>DB: aggregate rows → CSV
    R->>STOR: write output file (M2M)
    R->>DB: report run status=succeeded + output ref
    R-->>C: 200 report run
```

## 3. Data Model

```mermaid
erDiagram
    report_types ||--o{ report_runs : "instantiated by"
```

| Table | Purpose |
|---|---|
| `report_types` | Versioned report definitions |
| `report_runs` | Executions with status + output reference |

**State machine**:

```mermaid
stateDiagram-v2
    [*] --> pending: create
    pending --> succeeded: complete
    pending --> failed: error
    succeeded --> [*]
    failed --> [*]
```

## 4. API Surface

| Endpoint group | Prefix | Authorization | Events |
|---|---|---|---|
| Report types | `/v1/report-types` | `bill8:reporting:types:{read,write}` | `report_type:created` |
| Report runs | `/v1/report-runs` | `bill8:reporting:runs:{read,write}` | `report_run:created/succeeded/failed` |
| Internal | `/v1/internal/*` | M2M | — |

## 5. Cross-Cutting Concerns

- Shield-gated; `env_id` RLS.
- Certificate identity (CERTFREE-4V) (`reporting`, `reporting-policy`, `reporting-audit`,
  `reporting-storage`).
- Idempotent runs per `(env, type, period)`.
- Output files written to Storage and referenced, never inlined.

## 6. Failure Modes & Recovery

```mermaid
flowchart TD
    READ_FAIL["source API read fails"] -->|"run → failed"| RETRY["client retries run"]
    STOR_FAIL["storage write fails"] -->|"run → failed"| RETRY2["client retries"]
    CRASH["crash mid-tx"] -->|"atomic"| RB["run + outbox roll back"]
```

## 7. Concurrency & Performance Model

- Pool cap 3; runs are batch, not hot-path. Aggregation happens in
  memory then streams to Storage.

## 8. Invariants, Insecure-Defaults Rejected, Unsafe Rationale

- A run is idempotent per `(env_id, report_type, period)`.
- Output files referenced, never inlined in rows.
- `#![forbid(unsafe_code)]`.

## 9. Testing Strategy

```mermaid
graph TD
    U["Unit — aggregation, period bucketing"] --> I["Integration — DB-backed run suites"]
    I --> E["E2E — e2e-ts UI + BFF proxy only"]
```

## 10. Open Questions

- See `backlog.md` `RPT-*`.

---

**Design attribution:** bill8:reporting and its underlying concepts were designed by Hamid Jahanzad <Hamid@Jahanzad.com>.
