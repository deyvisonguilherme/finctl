-- Criação da tabela de contas
CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    name VARCHAR(100) NOT NULL,
    kind VARCHAR(20) NOT NULL CHECK (kind IN ('checking', 'savings', 'wallet', 'investment')),
    initial_balance NUMERIC(14,2) NOT NULL DEFAULT 0.00 CHECK (initial_balance >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_accounts_user_name UNIQUE (user_id, name)
);

-- Criação da tabela de categorias
CREATE TABLE IF NOT EXISTS categories (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    name VARCHAR(100) NOT NULL,
    kind VARCHAR(20) NOT NULL CHECK (kind IN ('income', 'expense')),
    parent_id UUID REFERENCES categories(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Índice único para evitar categorias duplicadas com mesmo nome para o mesmo usuário e pai
CREATE UNIQUE INDEX IF NOT EXISTS uq_categories_root_name 
    ON categories (user_id, name) 
    WHERE parent_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_categories_child_name 
    ON categories (user_id, parent_id, name) 
    WHERE parent_id IS NOT NULL;

-- Criação da tabela de lançamentos / transações
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    category_id UUID NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
    kind VARCHAR(20) NOT NULL CHECK (kind IN ('income', 'expense')),
    amount NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    date DATE NOT NULL,
    description VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Índices obrigatórios para buscas e relatórios eficientes
CREATE INDEX IF NOT EXISTS idx_transactions_account_date ON transactions(account_id, date);
CREATE INDEX IF NOT EXISTS idx_transactions_category_date ON transactions(category_id, date);
CREATE INDEX IF NOT EXISTS idx_transactions_user_date ON transactions(user_id, date);
