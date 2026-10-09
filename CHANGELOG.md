# Changelog

Todas as alterações notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.3.0] - 2026-10-09

### Adicionado
- **Interface Interativa em Terminal — TUI (F6-01 a F6-05, F6-08):**
  - Aplicação TUI completa baseada em arquitetura TEA (Elm) com `ratatui` e `crossterm`.
  - Processamento de banco de dados assíncrono em segundo plano via canais Tokio MPSC sem congelamento da interface.
  - Suporte a 5 abas integradas (`Dashboard`, `Lançamentos`, `Relatórios`, `Orçamentos`, `Metas e Projeção`).
  - Navegação global por teclado (`Tab`, `BackTab`, `1` a `5`, `q`, `r`, `t`, `?`).
  - Alternância dinâmica entre temas Escuro e Claro (`t` ou `--theme light|dark`).
  - Aba de Lançamentos com paginação, busca textual (`/`), modal de filtros avançados (`f`), formulário de cadastro/edição (`a`, `e`), exclusão com diálogo de confirmação (`d`) e marcação em lote como pago (`p`).
  - Aba de Relatórios com 3 sub-visões: Gastos por Categoria, Evolução Mensal com Sparklines e Comparativo entre Meses com variações percentuais.
  - Aba de Metas e Projeção com sub-visões de Metas de Economia (com gauges visuais de progresso e modal de aporte) e Projeção de Fluxo de Caixa (com sparkline e alerta de déficit).
  - Execução automática de migrações SQL pendentes na inicialização da TUI (`storage::run_migrations`).
- **Metas de Economia (F6-06):**
  - Migration `create_goals` e suporte a tabelas `goals` e `goal_contributions`.
  - Subcomandos `finctl goal add|list|show|contribute|edit|rm` com formatos `table`, `json` e `csv`.
  - Suporte a metas vinculadas a contas bancárias (progresso derivado do saldo) e metas manuais alimentadas por aportes.
  - Cálculo automático de aporte mensal necessário até a data alvo e estimativa de conclusão com base na média móvel dos últimos 90 dias.
  - Conclusão automática ao atingir 100% da meta e suporte a reabertura via `finctl goal edit --reopen`.
- **Projeção de Fluxo de Caixa (F6-07):**
  - Subcomando `finctl forecast [--months N] [--account X] [--granularity week|month] [--include-goals]`.
  - Projeção consolidada no regime de caixa considerando lançamentos pendentes, parcelas futuras, regras recorrentes após `last_generated_date` e faturas de cartão no vencimento (regra D-08).
  - Identificação e destaque automático do primeiro período deficitário com alerta `⚠️`.
  - Flag `--include-goals` para simulação opcional de saídas planejadas de metas ativas no fluxo de caixa.

### Modificado
- Remoção do corte arbitrário de 30 dias na listagem de lançamentos da TUI (`all_time: true`), permitindo navegar e paginar por todo o histórico (passado e lançamentos futuros agendados).
- Correção de duplicação do prefixo monetário `R$` na saída tabular de `finctl forecast`.
- Indicador do total de contas cadastradas no cabeçalho do painel de saldos do Dashboard da TUI.

## [0.2.0] - 2026-10-08

### Adicionado
- **Release e Empacotamento Multiplataforma (F5-05):**
  - Workflow de release no GitHub Actions com suporte para Linux (x86_64 e aarch64), macOS (Intel e Apple Silicon) e Windows (x86_64).
  - Geração de pacotes `.tar.gz` e `.zip` com somas de verificação SHA-256 (`.sha256`).
  - Suporte total a compilação offline com `SQLX_OFFLINE=true` e migrações SQL embutidas no binário via `sqlx::migrate!`.
  - Exibição de commit e data na flag `--version` (`finctl 0.2.0 (<commit> <data>)`).
- **Autocompletar e Man Pages (F5-04):**
  - Subcomando `finctl completions <bash|zsh|fish|powershell|elvish>` para geração rápida de autocompletar na saída padrão.
  - Subcomando `finctl man [--dir <pasta>]` para geração de páginas man roff UNIX.
  - Geração automática de todas as páginas `.1` no build de release via `build.rs`.
- **Backup e Restore (F5-03):**
  - Subcomandos `finctl backup` e `finctl restore` (com aliases sob `finctl db`).
  - Rotação com política de retenção (`--keep N`).
  - Gravação atômica (`.tmp` → `.dump`) e isolamento de credenciais via variáveis de subprocesso (`PGPASSWORD`, etc.).
  - Restauração protegida contra sobrescrita acidental do banco ativo (`--yes` e confirmação interativa).
- **Auditoria Imutável (F5-02):**
  - Tabela `audit_log` com triggers automáticos em transações, contas, categorias e orçamentos.
  - Rastreabilidade de autor via `SET LOCAL finctl.actor` e usuário do sistema operacional.
  - Subcomando `finctl audit list` com filtros por tabela, ID, data e formato de saída.
- **Soft Delete e Expurgo (F5-01):**
  - Exclusão lógica (`deleted_at`) em transações, contas e categorias.
  - Subcomandos `finctl tx restore`, `finctl tx list --deleted`, `finctl account rm`, `finctl category rm` e `finctl purge --older-than`.
- **Contas Avançadas e Conciliação (Fase 4):**
  - Transferências entre contas (`finctl transfer add`).
  - Cartão de crédito, faturas e fechamento/pagamento (`finctl card invoice`, `finctl card pay`).
  - Conciliação bancária assistida por CSV (`finctl reconcile`).
  - Tags e anexos transversais (`finctl tag`, `finctl tx attach`).
- **Planejamento e Orçamentos (Fase 3):**
  - Controle de status previsto × realizado (`paid` vs `pending`).
  - Orçamentos mensais e recorrentes por categoria (`finctl budget`).
  - Lançamentos recorrentes com motor de geração idempotente (`finctl recurring`).
  - Compras parceladas com desdobramento automático (`--installments`).
- **Relatórios e CSV (Fase 2):**
  - Relatórios mensais consolidados (`finctl report monthly`).
  - Relatórios por categoria com árvore hierárquica (`finctl report categories`).
  - Comparativo entre meses (`finctl report compare`).
  - Importação e exportação em CSV com perfis de banco e detecção de duplicidades.

## [0.1.0] - 2026-10-01

### Adicionado
- Fundação do sistema financeiro com Rust, Clap, PostgreSQL e Sqlx.
- Módulos `domain`, `storage`, `app` e `cli`.
- Suporte a dinheiro de precisão fixa com `rust_decimal` (`Money`).
- Subcomandos de inicialização (`finctl init`), contas (`finctl account`), categorias (`finctl category`), receitas (`finctl income`) e despesas (`finctl expense`).
- Consulta de saldos por conta e total consolidado (`finctl balance`).
- Containerização de desenvolvimento com `docker-compose.yml` e testes de isolamento com `testcontainers`.
