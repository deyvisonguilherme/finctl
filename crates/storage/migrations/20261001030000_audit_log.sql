-- Migration: Criação da tabela de auditoria e triggers de captura
-- Fase 5 - Task F5-02

-- 1. Criação da tabela audit_log
CREATE TABLE IF NOT EXISTS audit_log (
    id BIGSERIAL PRIMARY KEY,
    table_name VARCHAR(64) NOT NULL,
    row_id UUID NOT NULL,
    action VARCHAR(10) NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'DELETE')),
    old JSONB,
    new JSONB,
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor VARCHAR(128) NOT NULL
);

-- Índices para buscas rápidas por tabela, registro e data
CREATE INDEX IF NOT EXISTS idx_audit_log_table_row ON audit_log (table_name, row_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_changed_at ON audit_log (changed_at);

-- 2. Garantir imutabilidade (append-only)
CREATE OR REPLACE FUNCTION prevent_audit_log_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'audit_log é append-only: modificações e remoções não são permitidas';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_log_immutable ON audit_log;
CREATE TRIGGER trg_audit_log_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON audit_log
    FOR EACH STATEMENT EXECUTE FUNCTION prevent_audit_log_mutation();

REVOKE UPDATE, DELETE, TRUNCATE ON audit_log FROM PUBLIC;

-- 3. Função de trigger para registro de auditoria
CREATE OR REPLACE FUNCTION fn_audit_record_changes()
RETURNS TRIGGER AS $$
DECLARE
    v_actor TEXT;
    v_row_id UUID;
    v_old JSONB;
    v_new JSONB;
BEGIN
    -- Captura o ator definido na transação via SET LOCAL finctl.actor ou fallback para SESSION_USER
    v_actor := COALESCE(NULLIF(current_setting('finctl.actor', true), ''), SESSION_USER);

    IF (TG_OP = 'INSERT') THEN
        v_row_id := NEW.id;
        v_old := NULL;
        v_new := to_jsonb(NEW);
    ELSIF (TG_OP = 'UPDATE') THEN
        v_row_id := NEW.id;
        v_old := to_jsonb(OLD);
        v_new := to_jsonb(NEW);
    ELSIF (TG_OP = 'DELETE') THEN
        v_row_id := OLD.id;
        v_old := to_jsonb(OLD);
        v_new := NULL;
    END IF;

    INSERT INTO audit_log (table_name, row_id, action, old, new, changed_at, actor)
    VALUES (TG_TABLE_NAME, v_row_id, TG_OP, v_old, v_new, NOW(), v_actor);

    IF (TG_OP = 'DELETE') THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- 4. Triggers em transactions, accounts, categories e budgets
DROP TRIGGER IF EXISTS trg_audit_transactions ON transactions;
CREATE TRIGGER trg_audit_transactions
    AFTER INSERT OR UPDATE OR DELETE ON transactions
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();

DROP TRIGGER IF EXISTS trg_audit_accounts ON accounts;
CREATE TRIGGER trg_audit_accounts
    AFTER INSERT OR UPDATE OR DELETE ON accounts
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();

DROP TRIGGER IF EXISTS trg_audit_categories ON categories;
CREATE TRIGGER trg_audit_categories
    AFTER INSERT OR UPDATE OR DELETE ON categories
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();

DROP TRIGGER IF EXISTS trg_audit_budgets ON budgets;
CREATE TRIGGER trg_audit_budgets
    AFTER INSERT OR UPDATE OR DELETE ON budgets
    FOR EACH ROW EXECUTE FUNCTION fn_audit_record_changes();
