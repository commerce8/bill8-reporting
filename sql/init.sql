-- SPDX-FileCopyrightText: Copyright Logically Magic Inc. <hello@logicallymagic.com>
-- SPDX-FileCopyrightText: Copyright Hamid Jahanzad <Hamid@Jahanzad.com>
-- SPDX-FileCopyrightText: Copyright Contributors to the Bill8:Reporting
-- SPDX-License-Identifier: Apache-2.0

-- Bill8:Reporting - the service's own schema `reporting`: report
-- types and runs. Output files are Storage references, never inline.

CREATE SCHEMA IF NOT EXISTS reporting;

CREATE TABLE IF NOT EXISTS reporting.report_types (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    env_id      UUID NOT NULL,
    name        TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (env_id, name)
);
CREATE INDEX IF NOT EXISTS idx_reporting_report_types_env ON reporting.report_types (env_id);

CREATE TABLE IF NOT EXISTS reporting.report_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    env_id          UUID NOT NULL,
    report_type_id  UUID NOT NULL REFERENCES reporting.report_types(id),
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'succeeded', 'failed')),
    output_ref      TEXT,
    idempotency_key TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (env_id, report_type_id, period_start, period_end)
);
CREATE INDEX IF NOT EXISTS idx_reporting_report_runs_env ON reporting.report_runs (env_id);

-- ── Report schedules (RPT-3): a Cron schedule per (type, period) ────
CREATE TABLE IF NOT EXISTS reporting.report_schedules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    env_id          UUID NOT NULL,
    report_type_id  UUID NOT NULL REFERENCES reporting.report_types(id),
    interval        TEXT NOT NULL CHECK (interval IN ('daily', 'weekly', 'monthly')),
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reporting_report_schedules_env ON reporting.report_schedules (env_id);

-- ── Row-level security ────────────────────────────────────────────────
ALTER TABLE reporting.report_types ENABLE ROW LEVEL SECURITY;
ALTER TABLE reporting.report_types FORCE ROW LEVEL SECURITY;
ALTER TABLE reporting.report_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE reporting.report_runs FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON reporting.report_types USING (env_id = current_setting('app.env_id', true)::uuid);
CREATE POLICY tenant_isolation ON reporting.report_runs USING (env_id = current_setting('app.env_id', true)::uuid);
CREATE POLICY tenant_isolation ON reporting.report_schedules USING (env_id = current_setting('app.env_id', true)::uuid);

-- ── Producer tables ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS reporting.outbox (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    env_id              UUID NOT NULL,
    event_type          TEXT NOT NULL,
    stream_family       TEXT NOT NULL,
    payload             JSONB NOT NULL,
    publisher_id_token  TEXT,
    publisher_subject   TEXT,
    relayed             BOOLEAN NOT NULL DEFAULT false,
    relayed_at          TIMESTAMPTZ,
    relay_attempts      INT NOT NULL DEFAULT 0,
    next_retry_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reporting_outbox_unrelayed ON reporting.outbox (created_at) WHERE relayed = false;

CREATE OR REPLACE FUNCTION reporting.notify_outbox_changed() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('reporting_outbox_changed', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS reporting_outbox_notify ON reporting.outbox;
CREATE TRIGGER reporting_outbox_notify AFTER INSERT ON reporting.outbox
    FOR EACH ROW EXECUTE FUNCTION reporting.notify_outbox_changed();

CREATE TABLE IF NOT EXISTS reporting.audit_journal (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    env_id          UUID NOT NULL,
    action          TEXT NOT NULL,
    target_resource_type TEXT,
    target_resource_id    TEXT,
    actor_user_id   TEXT,
    actor_subject_type TEXT,
    severity        TEXT NOT NULL DEFAULT 'info',
    correlation_id  TEXT,
    before_state    JSONB,
    after_state     JSONB,
    details         JSONB,
    pushed          BOOLEAN NOT NULL DEFAULT false,
    pushed_at       TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reporting_audit_journal_unpushed
    ON reporting.audit_journal (created_at) WHERE pushed = false;
ALTER TABLE reporting.audit_journal ENABLE ROW LEVEL SECURITY;
ALTER TABLE reporting.audit_journal FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS audit_journal_dml ON reporting.audit_journal;
CREATE POLICY audit_journal_dml ON reporting.audit_journal
    USING (sentin8_context_initialized() AND env_id = sentin8_current_env_id())
    WITH CHECK (sentin8_context_initialized() AND env_id = sentin8_current_env_id());
-- ── Role grants (schema-ownership convention, bank8-ledger's form) ─────
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'bill8_reporting_service') THEN
        ALTER ROLE bill8_reporting_service SET search_path = reporting, public;
        GRANT USAGE ON SCHEMA reporting TO bill8_reporting_service;
        GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA reporting TO bill8_reporting_service;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA reporting TO bill8_reporting_service;
    END IF;
END $$;
