-- Migration: Soft delete para transactions, accounts e categories
-- Adiciona deleted_at e atualiza constraints de unicidade para índices parciais

-- 1. Colunas deleted_at
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE categories ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- 2. Atualização de restrições de unicidade em accounts
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS uq_accounts_user_name;
CREATE UNIQUE INDEX IF NOT EXISTS uq_accounts_user_name_active 
    ON accounts (user_id, name) 
    WHERE deleted_at IS NULL;

-- 3. Atualização de restrições de unicidade em categories
DROP INDEX IF EXISTS uq_categories_root_name;
CREATE UNIQUE INDEX IF NOT EXISTS uq_categories_root_name 
    ON categories (user_id, name) 
    WHERE parent_id IS NULL AND deleted_at IS NULL;

DROP INDEX IF EXISTS uq_categories_child_name;
CREATE UNIQUE INDEX IF NOT EXISTS uq_categories_child_name 
    ON categories (user_id, parent_id, name) 
    WHERE parent_id IS NOT NULL AND deleted_at IS NULL;

-- 4. Índices para consultas eficientes
CREATE INDEX IF NOT EXISTS idx_transactions_deleted_at ON transactions (user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_accounts_deleted_at ON accounts (user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_categories_deleted_at ON categories (user_id, deleted_at);
