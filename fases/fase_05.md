# Fase 5 — Robustez e release

Backlog da **Fase 5 — Robustez e release** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Regras transversais (a partir da Fase 5)

- **Soft delete:** toda consulta de leitura ignora registros com `deleted_at` preenchido. Centralize esse filtro (views ou um módulo único de condições) em vez de repeti-lo em cada query.
- **Fluxo de caixa × competência:** relatórios (Fase 2) usam competência (D-04). A projeção de fluxo de caixa (F6-07) usa **caixa**: compra no cartão sai na data de vencimento da fatura, não na data da compra.
- **Sem SQL na TUI:** a crate `tui` só chama a crate `app`, como a `cli`.

### Decisões a confirmar antes de iniciar

| ID | Decisão | Padrão sugerido |
|---|---|---|
| D-06 | Auditoria por trigger no banco ou no código da aplicação | Triggers no Postgres (capturam qualquer escrita, inclusive manual), com o ator vindo de `SET LOCAL finctl.actor` |
| D-07 | Progresso de uma meta | Meta pode ser vinculada a uma conta (progresso = saldo da conta) ou usar aportes manuais registrados na própria meta |
| D-08 | Projeção de fluxo de caixa para cartão | Saída na data de vencimento da fatura (regime de caixa) |
| D-09 | Modo de início padrão (`finctl` sem subcomando) | `ask`: pergunta TUI ou linha de comando em terminal interativo; fora dele, age como linha de comando |

---

## Tasks

### [x] F5-01 — Soft delete
- **Depende de:** G-01
- **Escopo:**
  - Migration adiciona `deleted_at TIMESTAMPTZ` em `transactions`, `accounts` e `categories`
  - Constraints de unicidade (ex.: nome de conta) viram índices únicos parciais `WHERE deleted_at IS NULL`
  - `tx rm` passa a ser soft delete; novos comandos `tx restore <id>`, `tx list --deleted` e `finctl purge --older-than <duração> [--yes]` (exclusão definitiva)
  - Remover conta ou categoria com lançamentos ativos é bloqueado com mensagem clara
- **Critérios de aceite:**
  - Saldo, relatórios, orçamentos, recorrências e fatura ignoram registros removidos
  - Restaurar uma transferência restaura as duas pontas; remover/restaurar um parcelamento por grupo funciona como `tx rm --group`
  - Há teste de regressão para cada consulta de leitura existente
  - `purge` exige confirmação e nunca atua em registros com menos tempo que o informado
- **Notas:** Implementada migration `20261001020000_soft_delete.sql` com conversão para índices parciais únicos (`WHERE deleted_at IS NULL`). Suporte a `deleted_at` em todas as consultas de leitura (saldos, relatórios mensais e de categoria, orçamentos e recorrências). Adicionados subcomandos `finctl tx restore <id> [--group]`, `finctl tx list --deleted`, `finctl account rm <account> [--yes]`, `finctl category rm <category> [--yes]` e `finctl purge --older-than <duração> [--yes]`. Criados testes completos de integração em `crates/storage/tests/soft_delete_test.rs`.

### [x] F5-02 — Auditoria
- **Depende de:** F5-01
- **Escopo:** tabela `audit_log` (id, tabela, `row_id`, ação `INSERT|UPDATE|DELETE`, `old` e `new` em `JSONB`, `changed_at`, `actor`). Triggers em `transactions`, `accounts`, `categories` e `budgets` (decisão D-06). A aplicação define `SET LOCAL finctl.actor` com o usuário do sistema operacional a cada transação. Comando `finctl audit list [--table X] [--id N] [--since <data>] [--format ...]`.
- **Critérios de aceite:**
  - Toda criação, edição e remoção (inclusive soft delete e restore) gera registro
  - Importação de 1.000 linhas continua com tempo aceitável (medir antes e depois dos triggers e registrar em Notas)
  - `audit_log` é somente de inserção para o papel usado pela aplicação (sem `UPDATE`/`DELETE` concedidos)
- **Notas:** Implementada migration `20261001030000_audit_log.sql` com tabela `audit_log` (`id`, `table_name`, `row_id`, `action`, `old`, `new`, `changed_at`, `actor`), trigger de imutabilidade (`prevent_audit_log_mutation`) e triggers `AFTER INSERT OR UPDATE OR DELETE` em `transactions`, `accounts`, `categories` e `budgets`. O ator é capturado via `SET LOCAL finctl.actor` com fallback para `SESSION_USER`. Mutação na aplicação envolvida em transações via `begin_tx`/`begin_with_actor` utilizando detecção de usuário do SO (variáveis de ambiente + crate `whoami`). Adicionado comando `finctl audit list [--table] [--id] [--since] [--limit] [--format table|json|csv]`. Medição de benchmark de importação de 1.000 linhas registrada: 1.12s sem triggers vs 1.11s com triggers (overhead nulo com transação atômica em batch). Testes de integração cobrindo criação, edição, soft-delete, restore, exclusão física e tentativas de mutação de audit_log em `crates/storage/tests/audit_test.rs`.

### [x] F5-03 — Backup e restore
- **Depende de:** G-01
- **Escopo:**
  - `finctl backup --output <dir> [--keep N]` executa `pg_dump` em formato custom (`-Fc`) com nome com data e hora e remove os backups mais antigos além de `N`
  - `finctl restore <arquivo> [--into <banco>] [--yes]` executa `pg_restore`; por padrão restaura em um banco novo, nunca sobre o atual sem `--yes` e confirmação digitando o nome do banco
  - Credenciais vêm de `DATABASE_URL`/variáveis de ambiente (`PGPASSWORD`), nunca em argumentos de linha de comando
- **Critérios de aceite:**
  - Erro claro quando `pg_dump`/`pg_restore` não estão no `PATH`
  - Teste de integração: backup → apagar dados → restore → saldos e relatórios idênticos
  - Backup interrompido não deixa arquivo parcial com nome final (escrever em temporário e renomear)
- **Notas:** Implementados comandos `finctl backup` e `finctl restore` (com aliases `finctl db backup` e `finctl db restore`). O backup grava inicialmente em arquivo `.tmp` e realiza rename atômico para `finctl_backup_YYYYMMDD_HHMMSS.dump`. Rotação via `--keep N` exclui apenas os arquivos mais antigos mantendo os `N` mais recentes. Credenciais e parâmetros de conexão são passados isoladamente por variáveis de ambiente de processo (`PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`, `PGDATABASE`), nunca via flags/argumentos de CLI. Se `--into` não for fornecido, cria automaticamente um banco isolado `<banco>_restore_YYYYMMDD_HHMMSS`. Restauração sobre o banco atual exige a flag `--yes` ou confirmação interativa com digitação do nome exato do banco. Mensagens claras com código de saída 2 caso `pg_dump` ou `pg_restore` não estejam disponíveis. Criada suíte completa de testes de integração em `crates/storage/tests/backup_restore_test.rs` validando fluxo completo, retenção e salvaguardas.

### [ ] F5-04 — Autocompletar e man page
- **Depende de:** G-01
- **Escopo:** `finctl completions <bash|zsh|fish|powershell>` com `clap_complete`; man page gerada com `clap_mangen`; revisão de consistência de todos os `--help` (português, flags com o mesmo nome em todos os comandos).
- **Critérios de aceite:**
  - O script gerado carrega sem erro em bash e zsh (teste de fumaça no CI)
  - Man page é gerada no build de release
  - Nenhum comando fica sem descrição no `--help`
- **Notas:**

### [ ] F5-05 — Release multiplataforma
- **Depende de:** F5-03, F5-04
- **Escopo:** workflow do GitHub Actions disparado por tag `vX.Y.Z` que compila para Linux (x86_64 e aarch64), macOS (x86_64 e arm64) e Windows (x86_64); `SQLX_OFFLINE=true` com `.sqlx/` versionado; migrations embutidas no binário (`sqlx::migrate!`); artefatos `.tar.gz`/`.zip` com SHA-256; `CHANGELOG.md`; versionamento semântico.
- **Critérios de aceite:**
  - Build de release não precisa de banco acessível
  - Cada artefato inclui binário, completions, man page e README
  - `finctl --version` mostra versão e commit
  - Release de teste (tag `v0.0.0-rc`) publica todos os artefatos
- **Notas:**

### [ ] F5-06 — Hardening de qualidade e desempenho
- **Depende de:** G-01
- **Escopo:**
  - Cobertura com `cargo-llvm-cov` no CI (meta: ≥ 80% em `domain` e `app`)
  - `cargo audit` e `cargo deny` no CI (vulnerabilidades e licenças)
  - Teste de carga com 100 mil lançamentos: revisar `EXPLAIN` de saldo, `tx list` e relatórios; criar os índices que faltarem
  - `finctl db status` mostra versão do schema e migrations pendentes
- **Critérios de aceite:**
  - CI falha se a cobertura de `domain`/`app` cair abaixo da meta
  - Saldo e relatório mensal respondem em tempo aceitável com 100 mil lançamentos (registrar os números em Notas)
  - Nenhuma consulta pesada faz *sequential scan* em `transactions` sem justificativa
- **Notas:**

---

## Gate — Fase 5

### [ ] G-02 — Gate da Fase 5
- **Depende de:** F5-01 a F5-06
- **Escopo:** fluxo completo de instalação a partir do artefato de release (baixar, migrar, usar, backup, restore) em máquina limpa; revisão das regras transversais.
- **Critérios de aceite:**
  - Cenário E2E com soft delete, auditoria e restore passa no CI
  - README documenta instalação, backup/restore e completions
- **Notas:**
