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

### [x] F1-03 — `finctl income add` e `finctl expense add`
- **Depende de:** F1-01, F1-02
- **Escopo:** registrar lançamentos com conta, categoria, valor, data (padrão: hoje) e descrição.
- **Critérios de aceite:**
  - `finctl expense add --account Nubank --category Mercado --amount 89,90 --date 2026-10-01 --desc "Compras"`
  - Aceita valor com vírgula ou ponto; rejeita valor ≤ 0
  - Categoria precisa ser compatível com o tipo (receita × despesa)
  - Imprime o ID do lançamento criado
- **Notas:** `Transaction` model, `TransactionRepository`, `TransactionService` e comandos `finctl income add` e `finctl expense add` implementados com validações de compatibilidade de categoria, formatação de moeda e datas.

### [x] F1-04 — `finctl tx list` com filtros
- **Depende de:** F1-03
- **Escopo:** listagem com filtros por período (`--from`, `--to`, `--month`), conta, categoria e tipo; ordenação por data.
- **Critérios de aceite:**
  - Filtros combináveis; sem filtros lista os últimos 30 dias
  - Suporta `--format table|json|csv` e `--limit`
  - Teste de integração cobre combinações de filtros
- **Notas:** `finctl tx list` implementado com QueryBuilder dinâmico, suporte a filtros combinados (`--from`, `--to`, `--month`, `--account`, `--category`, `--kind`, `--limit`), múltiplos formatos de saída e testes cobrindo os cenários.

### [x] F1-05 — `finctl tx edit|rm`
- **Depende de:** F1-03
- **Escopo:** editar campos de um lançamento por ID e removê-lo (com confirmação; `--yes` para pular).
- **Critérios de aceite:**
  - Edição valida as mesmas regras da criação
  - `rm` pede confirmação interativa por padrão
  - ID inexistente retorna erro claro (código `1`)
- **Notas:** `finctl tx edit` e `finctl tx rm` implementados com suporte a `--yes`, prompt interativo do `dialoguer` e validações completas de negócio.

### [x] F1-06 — `finctl balance`
- **Depende de:** F1-03
- **Escopo:** saldo por conta (saldo inicial + receitas − despesas) e total geral; opção `--at <data>` para saldo em uma data.
- **Critérios de aceite:**
  - Resultado confere com cálculo manual em teste de integração
  - Saída em tabela com total ao final; suporta `--format json`
- **Notas:** `finctl balance` implementado com agregação SQL (saldo inicial + receitas - despesas), suporte ao filtro `--at <data>`, exibição em tabela formatada com linha de TOTAL GERAL e exportação em json/csv.

### [x] F1-07 — Seed de categorias padrão
- **Depende de:** F1-02
- **Escopo:** `finctl init` cria categorias iniciais comuns (Moradia, Alimentação, Transporte, Saúde, Lazer, Salário, etc.).
- **Critérios de aceite:**
  - Comando é idempotente (rodar duas vezes não duplica)
  - Usuário pode pular com `--no-seed`
- **Notas:** `finctl init` implementado com execução de migrations e seed de categorias e subcategorias padrão de receitas e despesas de forma idempotente e suporte a flag `--no-seed`.

### [x] F1-08 — Documentação do MVP
- **Depende de:** F1-01 a F1-07
- **Escopo:** `README.md` com instalação, configuração, exemplos de uso de todos os comandos do MVP; `docs/specs/` atualizado.
- **Critérios de aceite:**
  - Um novo usuário consegue subir o banco, migrar e registrar o primeiro lançamento seguindo apenas o README
- **Notas:** `README.md` completo com instruções de instalação, Docker/Podman, inicialização, exemplos de todos os comandos CLI e documentação em `docs/specs/mvp.md`.

---

## Orquestração das Fases 2, 3 e 4

As três fases têm dependências cruzadas (relatórios precisam saber de status e transferências; cartão precisa de transferências e parcelas). Por isso o plano é **schema primeiro, depois trilhas paralelas por ondas**.

### Regras transversais (valem para todas as tasks das Fases 2–4)

- **Realizado × previsto:** saldo e relatórios consideram apenas `status = 'paid'` por padrão. Use `--include-pending` (relatórios) ou `--projected` (saldo) para incluir previstos.
- **Transferências:** saldo das contas inclui transferências; relatórios de receita/despesa **excluem** lançamentos com `transfer_id IS NOT NULL`.
- **Cartão de crédito:** a compra é despesa na **data da compra** (competência); o pagamento da fatura é uma **transferência** da conta pagadora para o cartão (decisão D-04).
- **Idempotência:** comandos que geram dados (`recurring run`, `import csv`) podem rodar duas vezes sem duplicar.
- **Operações compostas** (transferência, parcelamento, pagamento de fatura) rodam em uma única transação SQL.

## Fase 2 — Relatórios e consultas

### [x] F2-00 — Preparação de schema para as Fases 2–4
- **Depende de:** F1-08
- **Escopo:** migration única, **aditiva** (colunas nullable ou com default), para evitar migrations conflitantes entre trilhas:
  - `transactions.status` (`paid` | `pending`, default `paid`)
  - `transactions.transfer_id` (UUID, nullable)
  - `transactions.installment_group_id`, `installment_number`, `installment_total` (nullable)
  - `transactions.recurring_rule_id` (nullable)
  - `transactions.import_hash` (TEXT, nullable) com índice único parcial `(account_id, import_hash) WHERE import_hash IS NOT NULL`
  - `transactions.reconciled_at` (TIMESTAMPTZ, nullable)
  - `categories.is_system` (BOOLEAN, default `false`)
  - Domínio: enum `TransactionStatus`; repositórios atualizados para os novos campos
- **Critérios de aceite:**
  - A migration aplica sobre um banco com dados do MVP sem alterar nenhum comportamento existente
  - Todos os testes da Fase 1 continuam passando
  - `CHECK` garante coerência (`installment_number <= installment_total`, status válido)
- **Notas:** Migration aditiva `20261001010000_schema_phases_2_to_4.sql` criada, cobrindo colunas em transactions/categories/accounts e novas tabelas para orçamentos, regras de recorrência, faturas de cartão, tags e anexos. Repositórios e entidades de domínio atualizados. Todos os testes unitários e de integração passaram com sucesso.

### [x] F2-01 — Camada de agregação (storage/app)
- **Depende de:** F2-00
- **Escopo:** consultas e casos de uso reutilizáveis: totais por mês (receitas, despesas, saldo), totais por categoria (com opção de agregar subcategorias na categoria pai) e por período. Aplicam as regras transversais.
- **Critérios de aceite:**
  - Testes de integração com dataset fixo cobrem: lançamentos `pending` ignorados, transferências excluídas, subcategorias agregadas
  - Consultas usam os índices existentes (verificar com `EXPLAIN` em ao menos um teste ou na descrição do PR)
- **Notas:** `ReportRepository` e `ReportService` criados com agregações mensais, por categoria (com suporte a profundidade 1 para rollup em categoria pai e profundidade 2) e comparação temporal. Testes de integração cobrem exclusão de transferências, filtro de transações pendentes, rollup hierárquico e validação do plano via EXPLAIN.

### [x] F2-02 — `finctl report monthly`
- **Depende de:** F2-01
- **Escopo:** `finctl report monthly [--month YYYY-MM | --year YYYY] [--account X] [--include-pending]` mostra receitas, despesas e saldo do período (por mês, quando for ano).
- **Critérios de aceite:**
  - Padrão: mês corrente
  - Suporta `--format table|json|csv`
  - Valores conferem com cálculo manual em teste
- **Notas:** `finctl report monthly` implementado com suporte a `--month`, `--year`, `--account`, `--include-pending`, formatos table/json/csv e cálculo de taxa de poupança (savings rate). Testes de integração cobrem múltiplos cenários.

### [x] F2-03 — `finctl report categories`
- **Depende de:** F2-01
- **Escopo:** `finctl report categories [--month YYYY-MM] [--kind expense|income] [--depth 1|2]` lista total e percentual por categoria, ordenado do maior para o menor.
- **Critérios de aceite:**
  - Percentuais somam 100% (tratar arredondamento)
  - `--depth 1` agrupa subcategorias na categoria pai
  - Período sem lançamentos mostra mensagem amigável, não erro
- **Notas:** `finctl report categories` implementado com suporte a `--month`, `--kind`, `--depth 1|2`, `--account`, `--include-pending`, saídas formatadas (table, json, csv), mensagem amigável para períodos vazios e soma correta de percentuais com linha de TOTAL.

### [x] F2-04 — `finctl report compare`
- **Depende de:** F2-01
- **Escopo:** `finctl report compare --months 2026-08,2026-09` ou `--last N`; mostra por categoria o valor de cada mês e a variação absoluta e percentual.
- **Critérios de aceite:**
  - Variação com base zero não gera divisão por zero (exibe `n/d`)
  - Suporta `--format json`
- **Notas:** `finctl report compare` implementado com suporte a `--months` e `--last N`, cálculo de variação absoluta e percentual (com exibição segura de `n/d` em base zero sem divisão por zero), e saída em table/json/csv com destaque de cores. Testes de integração cobrem cenários com variação e base zero.

### [x] F2-05 — Exportação para arquivo
- **Depende de:** F1-04
- **Escopo:** `finctl export tx --output <arquivo> [filtros de tx list] [--format csv|json] [--locale pt-BR]`. No CSV pt-BR: separador `;`, vírgula decimal e BOM UTF-8 (abre direto no Excel).
- **Critérios de aceite:**
  - Reimportar o CSV exportado (F2-06, perfil `generic`) reproduz os mesmos lançamentos
  - Não sobrescreve arquivo existente sem `--force`
- **Notas:** `finctl export tx` implementado com suporte aos formatos CSV (pt-BR com BOM UTF-8, ';' e vírgula decimal; en-US com ',' e ponto decimal) e JSON, proteção contra sobrescrita acidental com flag `--force` e filtros completos de listagem de transações. Testes de integração validam a formatação do arquivo exportado.

### [x] F2-06 — Importação de CSV
- **Depende de:** F2-00
- **Escopo:** `finctl import csv <arquivo> --account X [--profile nome] [--dry-run]`. Perfis de mapeamento de colunas em TOML (`~/.config/finctl/profiles/`), com perfil `generic` embutido. `import_hash` calculado por data + valor + descrição + ocorrência. Lançamentos sem categoria vão para "A classificar".
- **Critérios de aceite:**
  - Rodar o mesmo arquivo duas vezes não duplica lançamentos
  - `--dry-run` mostra o que seria importado sem gravar
  - Resumo final: importados, duplicados ignorados e erros por linha (com número da linha)
  - Aceita vírgula ou ponto decimal e datas `DD/MM/YYYY` ou `YYYY-MM-DD`
- **Notas:** Comando `finctl import csv` implementado com suporte a perfis TOML (`generic`, `nubank` embutidos e diretório de perfis customizados), detecção inteligente de delimitador e formatos de data/valor monetário, cálculo de hash SHA-256 para garantia de idempotência, fallback para categoria de sistema "A classificar" / "A classificar (Receitas)", simulação via `--dry-run` e tabela de resumo com contagem de importados, duplicados e lista detalhada de erros por linha. Testes de integração cobrem idempotência, fallback e dry-run.

---

## Fase 3 — Planejamento

### [x] F3-01 — Status previsto × realizado
- **Depende de:** F2-00, F1-06
- **Escopo:** flag `--pending` em `income add`/`expense add`; `finctl tx pay <id> [--date]` marca como realizado; `tx list --status paid|pending`; `balance --projected [--at <data>]` inclui previstos até a data.
- **Critérios de aceite:**
  - `balance` e relatórios sem flags ignoram `pending`
  - `tx pay` em lançamento já pago retorna erro claro
  - Testes cobrem saldo realizado × projetado
- **Notas:** Implementado suporte a lançamentos previstos (`TransactionStatus::Pending`) e realizados (`TransactionStatus::Paid`). Flags `--pending` adicionadas a `income add` e `expense add`, comando `finctl tx pay <id> [--date]` para realização de transações, filtro `--status paid|pending` em `finctl tx list`, e cálculo de saldo projetado com `finctl balance --projected [--at <data>]`. Testes de integração validam fluxo completo de saldos e proteção contra pagamento duplicado.

### [x] F3-02 — Orçamentos por categoria
- **Depende de:** F2-01, F3-01
- **Escopo:** migration `budgets` (categoria, limite mensal, `month` opcional para exceção pontual). Comandos `budget set <categoria> <valor> [--month]`, `budget list`, `budget status [--month]` (consumido, restante, %, indicador `OK` / `ALERTA ≥ 80%` / `ESTOURADO`). Após `expense add`, exibir aviso se o orçamento da categoria foi ultrapassado.
- **Critérios de aceite:**
  - O aviso nunca altera o código de saída (continua `0`)
  - Orçamento da categoria pai considera as subcategorias
  - Testes cobrem os três estados do indicador
- **Notas:** Comandos `finctl budget set`, `list` e `status` implementados com suporte a orçamentos recorrentes e exceções mensais específicas. O status agrega despesas de categorias filhas na categoria pai e classifica os indicadores em OK, ALERTA (≥ 80%) e ESTOURADO (> 100%). Após `expense add`, alertas de orçamento são exibidos no stderr sem alterar o exit code 0. Testes de integração validam hierarquia, cálculo de status e alertas.

### [x] F3-03 — Regras de recorrência (CRUD)
- **Depende de:** F2-00
- **Escopo:** migration `recurring_rules` (tipo, conta, categoria, valor, descrição, frequência `weekly|monthly|yearly`, dia, `start_date`, `end_date`, `active`, `last_generated_date`). Comandos `recurring add|list|edit|pause|rm`.
- **Critérios de aceite:**
  - Dia 29–31 em mês mais curto usa o último dia do mês (decisão D-05)
  - Validações iguais às de lançamentos avulsos
  - `rm` não apaga lançamentos já gerados
- **Notas:** Comandos `finctl recurring add`, `list`, `edit`, `pause`, `resume` e `rm` implementados para regras semanais, mensais e anuais. Validações de contas, categorias e valores aplicadas de acordo com as regras de domínio. `recurring rm` remove a regra sem apagar os lançamentos já gerados. Testes de integração cobrem ciclo completo de CRUD, pausa, retomada e persistência.

### [x] F3-04 — `finctl recurring run`
- **Depende de:** F3-03, F3-01
- **Escopo:** `finctl recurring run [--until <data>] [--dry-run]` gera lançamentos `pending` com `recurring_rule_id`. Índice único `(recurring_rule_id, date)` garante idempotência. Documentar exemplo de agendamento via cron ou systemd timer.
- **Critérios de aceite:**
  - Rodar duas vezes seguidas não duplica lançamentos
  - Recupera ocorrências atrasadas (ex.: não rodou por 2 meses)
  - Falha parcial reverte tudo (transação única)
- **Notas:** Comando `finctl recurring run` implementado com suporte a `--until`, `--dry-run` e formatos de saída table/json/csv. Processamento executado atomicamente em transação SQL única, gerando lançamentos em status `pending` e atualizando `last_generated_date`. Cálculo de datas ajusta automaticamente meses curtos (decisão D-05) e recupera ocorrências em atraso. Idempotência garantida tanto em memória quanto pelo índice único `uq_transactions_recurring_date`. Testes unitários e de integração cobrem ciclo completo, simulação e idempotência.

  *Exemplo de agendamento via cron:*
  ```cron
  0 6 * * * finctl recurring run >> /var/log/finctl-recurring.log 2>&1
  ```
  *Exemplo de agendamento via systemd timer (`~/.config/systemd/user/finctl-recurring.timer`):*
  ```ini
  [Timer]
  OnCalendar=daily
  Persistent=true
  ```

### [x] F3-05 — Parcelamentos
- **Depende de:** F3-01
- **Escopo:** `expense add --installments N` (com valor total ou `--installment-amount`) gera N lançamentos `pending`, com datas mensais e mesmo `installment_group_id`. Centavos de arredondamento vão para a primeira parcela. `tx list --group <id>`, `tx edit --group` e `tx rm --group` atuam só nas parcelas ainda `pending`.
- **Critérios de aceite:**
  - A soma das parcelas é exatamente o valor total
  - Dia 29–31 segue a regra D-05
  - Parcelas já pagas nunca são alteradas ou removidas pelos comandos de grupo
- **Notas:** Suporte a compras parceladas implementado via `expense add --installments N` (com valor total ou `--installment-amount`). As parcelas são criadas em lote com status `pending`, vinculadas pelo mesmo `installment_group_id` e numeradas sequencialmente (`installment_number` e `installment_total`). A divisão de valores direciona eventuais centavos de arredondamento para a 1ª parcela garantindo soma exata, e datas respeitam o ajuste para meses mais curtos (D-05). Os comandos `finctl tx list --group <id>`, `finctl tx edit --group <id>` e `finctl tx rm --group <id>` atuam estritamente sobre as parcelas ainda pendentes, preservando parcelas pagas intactas. Testes de integração cobrem criação, divisão, filtros e operações em grupo.

---

## Fase 4 — Contas avançadas

### [x] F4-01 — Transferências entre contas
- **Depende de:** F2-00, F1-06
- **Escopo:** `finctl transfer add --from A --to B --amount V [--date]` cria duas linhas em uma transação SQL (despesa na origem, receita no destino) com o mesmo `transfer_id`, usando a categoria de sistema "Transferência" (`is_system = true`, uma por tipo). `tx rm` de uma perna remove a outra.
- **Critérios de aceite:**
  - Origem e destino devem ser contas diferentes
  - Saldos das contas refletem a transferência; relatórios de receita/despesa não
  - `tx edit` em lançamento de transferência é bloqueado com mensagem orientando a refazer a transferência
- **Notas:** Implementado comando `finctl transfer add` com criação atômica das pernas de débito (origem) e crédito (destino) vinculadas pelo mesmo `transfer_id` e categorias de sistema "Transferência" e "Transferência (Receita)". Os saldos das contas refletem a movimentação, enquanto relatórios agregados e categorizados excluem lançamentos com `transfer_id IS NOT NULL`. A edição individual (`tx edit`) de qualquer perna é bloqueada por validação para preservar a integridade contábil, e a exclusão (`tx rm`) de uma perna remove automaticamente a contraparte vinculada. Testes de integração validam o fluxo completo, isolamento de relatórios e proteção contra edição.

### [x] F4-02 — Cartão de crédito: modelo e faturas
- **Depende de:** F4-01, F3-01
- **Escopo:** `accounts.kind = credit_card` com `closing_day`, `due_day` e `credit_limit` opcional; migration `card_invoices` (conta, mês de referência, datas de fechamento e vencimento, status `open|closed|paid`); atribuição automática da compra à fatura (compra após o fechamento vai para a fatura seguinte); parcelas (F3-05) distribuídas nas faturas seguintes.
- **Critérios de aceite:**
  - Compra no dia do fechamento e no dia seguinte caem em faturas diferentes (teste explícito)
  - Faturas são criadas sob demanda, sem duplicar
  - Parcelamento de N parcelas gera lançamentos em N faturas consecutivas
- **Notas:** Implementado suporte a contas de cartão de crédito (`AccountKind::CreditCard`) com dia de fechamento (`closing_day`), dia de vencimento (`due_day`) e limite (`credit_limit`). A entidade `CardInvoice` e o repositório `CardInvoiceRepository` gerenciam os ciclos de faturamento e criam faturas sob demanda (`get_or_create_for_transaction`). Compras realizadas até a data de fechamento pertencem à fatura do mês atual, enquanto compras após o fechamento avançam automaticamente para a fatura seguinte (com ajuste de meses mais curtos D-05). Parcelamentos distribuem as parcelas em faturas consecutivas. Testes unitários e de integração validam todas as regras de atribuição e persistência.

### [x] F4-03 — `finctl card invoice`
- **Depende de:** F4-02
- **Escopo:** `card invoice list|show <cartão> [--month]` exibe lançamentos, total, vencimento e limite disponível; `card invoice close <cartão> [--month]` fecha a fatura.
- **Critérios de aceite:**
  - Fatura fechada não aceita novos lançamentos (eles vão para a próxima)
  - Total da fatura confere com a soma dos lançamentos
- **Notas:** Comandos `finctl card invoice list`, `show` e `close` implementados com saída em table, json e csv. A listagem totaliza os lançamentos e indica o status de cada ciclo. O comando show detalha as despesas do período, vencimento, limite total e limite disponível. O fechamento (`close`) atualiza o status da fatura para `closed` e direciona lançamentos futuros ou atrasados automaticamente para a próxima fatura aberta. Testes de integração validam totais, limites e comportamento de rollover.

### [x] F4-04 — Pagamento de fatura
- **Depende de:** F4-03, F4-01
- **Escopo:** `card pay <cartão> --from <conta> [--month] [--amount]` gera uma transferência (F4-01) da conta pagadora para o cartão e atualiza o status da fatura. Valor padrão: total da fatura.
- **Critérios de aceite:**
  - Pagamento total marca a fatura como `paid`
  - Pagamento parcial mantém `closed`, com saldo restante visível em `card invoice show`
  - Pagamento não aparece como despesa nos relatórios (é transferência)
- **Notas:** Implementado comando `finctl card pay` para liquidação total ou parcial de faturas de cartão de crédito. A operação executa uma transferência atômica da conta pagadora para a conta do cartão com identificação do ciclo da fatura na descrição. Pagamentos parciais mantêm a fatura em status `closed` e exibem o saldo remanescente em `card invoice show`, enquanto pagamentos integrais atualizam o status para `paid`. Por se tratar de transferência contábil, os pagamentos não são computados como despesas em relatórios. Testes de integração validam fluxo de pagamento total, parcial e isolamento de relatórios.

### [x] F4-05 — Conciliação com extrato
- **Depende de:** F2-06
- **Escopo:** `finctl reconcile --account X --file extrato.csv [--profile nome]` casa lançamentos por valor, data (± N dias, padrão 3) e similaridade de descrição; apresenta os pares sugeridos para confirmação e grava `reconciled_at`. `finctl reconcile status [--account X]` lista os não conciliados.
- **Critérios de aceite:**
  - Reutiliza o parser e os perfis da F2-06
  - Nada é gravado sem confirmação (ou `--yes`)
  - Itens do extrato sem correspondência são listados, sem criar lançamentos automaticamente
- **Notas:** Implementado serviço `ReconcileService` e comando `finctl reconcile` com matching ponderado por proximidade de datas e similaridade textual de descrição. Apresenta proposta categorizada em `[MATCH]`, `[NOVO]` e `[PENDENTE]`, permitindo simulação e aplicação com `--yes` ou confirmação interativa. O comando `finctl reconcile status` lista os lançamentos pendentes com suporte a formatos `table`, `json` e `csv`. Testes de integração validam fluxo de matching, janelas de tolerância e atualização de `reconciled_at`.

### [ ] F4-06 — Tags e referência de anexos
- **Depende de:** F2-00
- **Escopo:** tabelas `tags`, `transaction_tags` e `attachments` (transação, URI, SHA-256 opcional, nota). Comandos `tag add|list|rm`, `tx tag <id> <tags...>`, `tx list --tag`, `tx attach <id> <caminho|url>` (só guarda a referência; não armazena o arquivo).
- **Critérios de aceite:**
  - Anexo local tem a existência verificada e o hash calculado
  - `tag rm` pede confirmação quando a tag está em uso
  - `report categories --tag` filtra por tag, se F2-03 estiver concluída
- **Notas:**

---

## Gate — Revisão das Fases 2–4

### [ ] G-01 — Revisão e documentação
- **Depende de:** todas as tasks das Fases 2–4
- **Escopo:** teste de ponta a ponta (conta → lançamentos → recorrência → parcelas → cartão → pagamento de fatura → relatórios), README atualizado com os novos comandos e revisão das regras transversais.
- **Critérios de aceite:**
  - Cenário E2E automatizado passa no CI
  - Relatórios nunca contam transferências nem lançamentos `pending` por padrão
  - README cobre todos os comandos novos
- **Notas:**

---

## Backlog (fases futuras — detalhar antes de iniciar)

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
