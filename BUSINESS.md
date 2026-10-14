# Bill8:Reporting — Business Requirements & Functionality

> Audience: product owners, business analysts, and anyone evaluating what
> Bill8:Reporting **does for the business**. For architecture read
> `README.md`; for wire-level detail read `DESIGN.md`.

## 1. Business Purpose

Bill8:Reporting is the **reporting surface**. It delivers:

- **Consistent reports** — report types define a query shape, and runs
  snapshot data at a point in time.
- **Scheduled delivery** — scheduled reports deliver output files.

## 2. Business Scope (What the Service Provides)

| # | Capability | What the business gets |
|---|---|---|
| 1 | Report types | Versioned report definitions |
| 2 | Report runs | Executions with status + output reference |

**What the service does NOT own (business boundary):**

| Out of scope | Owned by |
|---|---|
| Underlying data | Owning services (read via public API) |
| Object storage | Sentin8:Storage |
| Scheduling | Sentin8:Cron |

## 3. Business Capabilities

| Capability | Business rule |
|---|---|
| Run a report | A run snapshots data for a period |
| Schedule a report | A scheduled run delivers output files |

## 4. Business Events (Outbound)

| Event | Fires when |
|---|---|
| `report_type:created` | A report type is defined |
| `report_run:created/succeeded/failed` | A run lifecycle changes |

## 5. Business Rules & Invariants

1. A run is idempotent per `(env_id, report_type, period)`.
2. Output files are referenced, never inlined.

## 6. Target-User Roles & Use Cases

| Persona | Does |
|---|---|
| Merchant | Run and download reports |
| Operator | Schedule period-end reports |

## 7. KPIs / Success Criteria

| KPI | Measure |
|---|---|
| Run success | succeeded / attempted |

## 8. Dependencies & Reliance on Other Services

| Reliance | On |
|---|---|
| Data reads | Bill8:Payments, Bill8:Ledger |
| Object storage | Sentin8:Storage |
| Scheduling | Sentin8:Cron |

## 9. Acceptance Criteria

1. A merchant can run a report and retrieve its output.
2. A scheduled report delivers output files on schedule.

## 10. Open Business Questions

- See `backlog.md` `RPT-*`.

---

**Design attribution:** bill8:reporting and its underlying concepts were designed by Hamid Jahanzad <Hamid@Jahanzad.com>.
