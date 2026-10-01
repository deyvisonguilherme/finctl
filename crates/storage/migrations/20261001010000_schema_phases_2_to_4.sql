-- Migration aditiva para suporte às Fases 2, 3 e 4

-- 1. Atualização de contas
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_kind_check;
ALTER TABLE accounts ADD CONSTRAINT accounts_kind_check 
    CHECK (kind IN ('checking', 'savings', 'wallet', 'investment', 'credit_card'));

ALTER TABLE accounts ADD COLUMN IF NOT EXISTS closing_day INT CHECK (closing_day BETWEEN 1 AND 31);
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS due_day INT CHECK (due_day BETWEEN 1 AND 31);
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS credit_limit NUMERIC(14,2) CHECK (credit_limit >= 0);

-- 2. Atualização de categorias
ALTER TABLE categories ADD COLUMN IF NOT EXISTS is_system BOOLEAN NOT NULL DEFAULT false;

-- 3. Atualização de transações
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'paid' CHECK (status IN ('paid', 'pending'));
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS transfer_id UUID;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS installment_group_id UUID;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS installment_number INT CHECK (installment_number IS NULL OR installment_number >= 1);
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS installment_total INT CHECK (installment_total IS NULL OR installment_total >= 1);
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS recurring_rule_id UUID;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS import_hash VARCHAR(64);
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS reconciled_at TIMESTAMPTZ;

ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_installment_coherence;
ALTER TABLE transactions ADD CONSTRAINT chk_installment_coherence 
    CHECK ((installment_number IS NULL AND installment_total IS NULL) OR (installment_number <= installment_total));

CREATE UNIQUE INDEX IF NOT EXISTS uq_transactions_account_import_hash 
    ON transactions (account_id, import_hash) 
    WHERE import_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions (user_id, status);
CREATE INDEX IF NOT EXISTS idx_transactions_transfer_id ON transactions (transfer_id) WHERE transfer_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_transactions_installment_group ON transactions (installment_group_id) WHERE installment_group_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_transactions_recurring_rule ON transactions (recurring_rule_id) WHERE recurring_rule_id IS NOT NULL;

-- 4. Tabelas de Orçamento (Budgets)
CREATE TABLE IF NOT EXISTS budgets (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    category_id UUID NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    amount NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    month VARCHAR(7),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_budgets_category_annual 
    ON budgets (user_id, category_id) 
    WHERE month IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_budgets_category_monthly 
    ON budgets (user_id, category_id, month) 
    WHERE month IS NOT NULL;

-- 5. Tabelas de Recorrência (Recurring Rules)
CREATE TABLE IF NOT EXISTS recurring_rules (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    kind VARCHAR(20) NOT NULL CHECK (kind IN ('income', 'expense')),
    amount NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    description VARCHAR(255) NOT NULL,
    frequency VARCHAR(20) NOT NULL CHECK (frequency IN ('weekly', 'monthly', 'yearly')),
    day_of_month INT CHECK (day_of_month BETWEEN 1 AND 31),
    day_of_week INT CHECK (day_of_week BETWEEN 1 AND 7),
    start_date DATE NOT NULL,
    end_date DATE,
    active BOOLEAN NOT NULL DEFAULT true,
    last_generated_date DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_transactions_recurring_date 
    ON transactions (recurring_rule_id, date) 
    WHERE recurring_rule_id IS NOT NULL;

-- 6. Tabelas de Faturas de Cartão de Crédito (Card Invoices)
CREATE TABLE IF NOT EXISTS card_invoices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    month VARCHAR(7) NOT NULL,
    closing_date DATE NOT NULL,
    due_date DATE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'paid')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_card_invoices_account_month UNIQUE (account_id, month)
);

-- 7. Tabelas de Tags e Anexos
CREATE TABLE IF NOT EXISTS tags (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    name VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_tags_user_name UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS transaction_tags (
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (transaction_id, tag_id)
);

CREATE TABLE IF NOT EXISTS attachments (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    uri TEXT NOT NULL,
    sha256 VARCHAR(64),
    note VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
