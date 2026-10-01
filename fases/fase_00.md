# Fase 0 — Fundação

Backlog da **Fase 0 — Fundação** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Tasks

### [x] F0-01 — Criar workspace Cargo e CI
- **Depende de:** —
- **Escopo:** workspace com as crates `domain`, `storage`, `app` e `cli`; `rustfmt.toml` e `clippy` configurados; pipeline de CI (fmt, clippy, test).
- **Critérios de aceite:**
  - `cargo build --workspace` compila sem warnings
  - CI roda fmt, clippy (`-D warnings`) e test
  - `finctl --version` imprime a versão
- **Notas:** Workspace Cargo inicializado com crates `domain`, `storage`, `app`, `cli`, rustfmt.toml e CI GitHub Actions configurados.

### [x] F0-02 — Ambiente de desenvolvimento com PostgreSQL
- **Depende de:** F0-01
- **Escopo:** `docker-compose.yml` (compatível com Podman) com Postgres, `.env.example`, leitura de `DATABASE_URL`.
- **Critérios de aceite:**
  - `docker compose up -d` (ou `podman compose up -d`) sobe o banco
  - `.env` é ignorado pelo git; `.env.example` documenta as variáveis
  - Binário falha com mensagem clara se `DATABASE_URL` estiver ausente
- **Notas:** `docker-compose.yml` criado com Postgres 16 Alpine, `.env.example` documentado e verificação de `DATABASE_URL` com código de saída 2 implementada.

### [x] F0-03 — Conexão, pool e tratamento de erros
- **Depende de:** F0-02
- **Escopo:** módulo de conexão com `sqlx::PgPool` na crate `storage`; tipos de erro (`thiserror`) no `domain`/`app`; `anyhow` no binário; `tracing` inicializado.
- **Critérios de aceite:**
  - Comando `finctl db ping` confirma a conexão
  - Erro de conexão retorna código de saída `2` com mensagem clara
- **Notas:** `sqlx::PgPool` e `ping` implementados em `storage`, tipos de erro em `domain`/`app`, `finctl db ping` e tracing adicionados com retorno de código de saída 2 em erros de conexão.

### [x] F0-04 — Value object `Money` e tipos base do domínio
- **Depende de:** F0-01
- **Escopo:** `Money` (wrapper de `Decimal`, 2 casas, sempre positivo), `TransactionKind` (`Income`/`Expense`), `AccountKind`, IDs tipados.
- **Critérios de aceite:**
  - `Money` rejeita valores negativos, zero (se aplicável) e mais de 2 casas decimais
  - Testes unitários cobrem criação, soma e formatação (`R$ 1.234,56`)
  - Nenhum uso de `f64` para dinheiro
- **Notas:** `Money` com validações e formatação pt-BR, `TransactionKind`, `AccountKind` e IDs tipados (`AccountId`, `CategoryId`, `TransactionId`, `UserId`) implementados no `domain` com testes unitários cobrindo todos os critérios.

### [x] F0-05 — Primeira migration: accounts, categories, transactions
- **Depende de:** F0-03
- **Escopo:** schema inicial conforme `AGENTS.md` (decisões D-01 e D-03).
  - `accounts` (id, [user_id], nome, tipo, saldo_inicial, created_at, updated_at)
  - `categories` (id, [user_id], nome, tipo, parent_id, created_at)
  - `transactions` (id, [user_id], account_id, category_id, tipo, valor `NUMERIC(14,2)`, data `DATE`, descrição, created_at, updated_at)
  - Constraints: `valor > 0`, FKs, `tipo` com `CHECK`
  - Índices em `(account_id, date)` e `(category_id, date)`
- **Critérios de aceite:**
  - `finctl db migrate` aplica as migrations em banco vazio
  - Constraints impedem valor ≤ 0 e tipo inválido
  - Teste de integração com `testcontainers` valida o schema
- **Notas:** Migration inicial criada em `crates/storage/migrations`, comando `finctl db migrate` adicionado e testes de integração com `testcontainers` validando schema e constraints em banco PostgreSQL real.

### [x] F0-06 — Infraestrutura de testes de integração
- **Depende de:** F0-05
- **Escopo:** helper de teste que sobe Postgres via `testcontainers`, aplica migrations e devolve um pool isolado por teste.
- **Critérios de aceite:**
  - `cargo test --workspace` roda os testes de integração localmente e no CI
  - Testes não interferem entre si
- **Notas:** `TestDb` implementado em `crates/storage/src/test_helpers.rs` compatível com Docker/Podman, com isolamento garantido por container/usuário e testes em `crates/storage/tests/isolation_test.rs`.
