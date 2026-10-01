# TASKS.md — finctl

Backlog de tasks do projeto. Regras de uso estão em `AGENTS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

**Formato de cada task:** `ID — título`, com **Depende de**, **Escopo**, **Critérios de aceite** e **Notas**.

---

## Fase 0 — Fundação

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

---

## Fase 1 — MVP: receitas e despesas

### [x] F1-01 — `finctl account add|list`
- **Depende de:** F0-05, F0-04
- **Escopo:** caso de uso + repositório + comandos; campos: nome, tipo (corrente, poupança, carteira, investimento), saldo inicial.
- **Critérios de aceite:**
  - `finctl account add "Nubank" --kind checking --initial-balance 1500,00` cria a conta
  - `finctl account list` exibe tabela; suporta `--format json|csv`
  - Nome duplicado retorna erro de validação (código `1`)
- **Notas:** `Account` model, `AccountRepository`, `AccountService` e comandos `finctl account add|list` implementados com suporte a `--format table|json|csv` e validações de unicidade por usuário.

### [x] F1-02 — `finctl category add|list`
- **Depende de:** F0-05
- **Escopo:** categorias de receita e despesa, com categoria pai opcional.
- **Critérios de aceite:**
  - `finctl category add "Mercado" --kind expense [--parent "Alimentação"]`
  - Categoria pai deve ter o mesmo `tipo` da filha
  - `finctl category list` mostra a hierarquia
- **Notas:** `Category` model, `CategoryRepository`, `CategoryService` com validações de consistência de tipo entre pai e filha, limite de 2 níveis de profundidade e comando `finctl category add|list` com exibição em árvore.

### [ ] F1-03 — `finctl income add` e `finctl expense add`
- **Depende de:** F1-01, F1-02
- **Escopo:** registrar lançamentos com conta, categoria, valor, data (padrão: hoje) e descrição.
- **Critérios de aceite:**
  - `finctl expense add --account Nubank --category Mercado --amount 89,90 --date 2026-10-01 --desc "Compras"`
  - Aceita valor com vírgula ou ponto; rejeita valor ≤ 0
  - Categoria precisa ser compatível com o tipo (receita × despesa)
  - Imprime o ID do lançamento criado
- **Notas:**

### [ ] F1-04 — `finctl tx list` com filtros
- **Depende de:** F1-03
- **Escopo:** listagem com filtros por período (`--from`, `--to`, `--month`), conta, categoria e tipo; ordenação por data.
- **Critérios de aceite:**
  - Filtros combináveis; sem filtros lista os últimos 30 dias
  - Suporta `--format table|json|csv` e `--limit`
  - Teste de integração cobre combinações de filtros
- **Notas:**

### [ ] F1-05 — `finctl tx edit|rm`
- **Depende de:** F1-03
- **Escopo:** editar campos de um lançamento por ID e removê-lo (com confirmação; `--yes` para pular).
- **Critérios de aceite:**
  - Edição valida as mesmas regras da criação
  - `rm` pede confirmação interativa por padrão
  - ID inexistente retorna erro claro (código `1`)
- **Notas:**

### [ ] F1-06 — `finctl balance`
- **Depende de:** F1-03
- **Escopo:** saldo por conta (saldo inicial + receitas − despesas) e total geral; opção `--at <data>` para saldo em uma data.
- **Critérios de aceite:**
  - Resultado confere com cálculo manual em teste de integração
  - Saída em tabela com total ao final; suporta `--format json`
- **Notas:**

### [ ] F1-07 — Seed de categorias padrão
- **Depende de:** F1-02
- **Escopo:** `finctl init` cria categorias iniciais comuns (Moradia, Alimentação, Transporte, Saúde, Lazer, Salário, etc.).
- **Critérios de aceite:**
  - Comando é idempotente (rodar duas vezes não duplica)
  - Usuário pode pular com `--no-seed`
- **Notas:**

### [ ] F1-08 — Documentação do MVP
- **Depende de:** F1-01 a F1-07
- **Escopo:** `README.md` com instalação, configuração, exemplos de uso de todos os comandos do MVP; `docs/specs/` atualizado.
- **Critérios de aceite:**
  - Um novo usuário consegue subir o banco, migrar e registrar o primeiro lançamento seguindo apenas o README
- **Notas:**

---

## Backlog (fases futuras — detalhar antes de iniciar)

### Fase 2 — Relatórios e consultas
- [ ] Resumo mensal: receitas × despesas × saldo
- [ ] Gastos por categoria (com percentual)
- [ ] Comparativo entre meses
- [ ] Exportação CSV/JSON
- [ ] Importação de CSV (extratos bancários)

### Fase 3 — Planejamento
- [ ] Orçamentos por categoria com alerta de limite (`finctl budget set|status`)
- [ ] Transações recorrentes (`finctl recurring add|run`)
- [ ] Parcelamentos
- [ ] Status previsto × realizado (`pending`/`paid`)

### Fase 4 — Contas avançadas
- [ ] Transferências entre contas (`transfer_id`)
- [ ] Cartão de crédito: fatura, fechamento e vencimento
- [ ] Conciliação com extrato
- [ ] Tags e referência de anexos

### Fase 5 — Robustez e release
- [ ] Soft delete e auditoria
- [ ] `finctl backup` / `restore` via `pg_dump`
- [ ] Binários multiplataforma via GitHub Actions
- [ ] Autocompletar de shell (`clap_complete`)

### Ideias (sem prioridade)
- [ ] TUI com `ratatui`
- [ ] API HTTP com `axum` reaproveitando `app` e `storage`
- [ ] Multi-moeda
- [ ] Metas de economia e projeção de fluxo de caixa
