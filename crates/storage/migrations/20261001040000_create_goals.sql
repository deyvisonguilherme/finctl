-- Migration: Criação das tabelas de metas de economia (goals) e aportes (goal_contributions)
-- Fase 6 - Task F6-06

-- 1. Tabela goals
CREATE TABLE IF NOT EXISTS goals (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    name VARCHAR(100) NOT NULL,
    target_amount NUMERIC(14,2) NOT NULL CHECK (target_amount > 0),
    target_date DATE,
    account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

-- Unicidade do nome da meta ativa por usuário
CREATE UNIQUE INDEX IF NOT EXISTS uq_goals_user_name_active
    ON goals (user_id, name)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_goals_user_id ON goals (user_id);
CREATE INDEX IF NOT EXISTS idx_goals_account_id ON goals (account_id);
CREATE INDEX IF NOT EXISTS idx_goals_deleted_at ON goals (user_id, deleted_at);

-- 2. Tabela goal_contributions
CREATE TABLE IF NOT EXISTS goal_contributions (
    id UUID PRIMARY KEY,
    goal_id UUID NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    amount NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    date DATE NOT NULL,
    note VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_goal_contributions_goal_id ON goal_contributions (goal_id);
CREATE INDEX IF NOT EXISTS idx_goal_contributions_user_date ON goal_contributions (user_id, date);
CREATE INDEX IF NOT EXISTS idx_goal_contributions_deleted_at ON goal_contributions (user_id, deleted_at);

-- 3. Triggers de auditoria
DROP TRIGGER IF EXISTS trg_audit_goals ON goals;
CREATE TRIGGER trg_audit_goals
    AFTER INSERT OR UPDATE OR DELETE ON goals
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();

DROP TRIGGER IF EXISTS trg_audit_goal_contributions ON goal_contributions;
CREATE TRIGGER trg_audit_goal_contributions
    AFTER INSERT OR UPDATE OR DELETE ON goal_contributions
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();
