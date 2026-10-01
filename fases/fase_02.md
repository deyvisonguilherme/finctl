# Fase 2 — Relatórios e consultas

Backlog da **Fase 2 — Relatórios e consultas** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Regras transversais (Fases 2–4)

- **Realizado × previsto:** saldo e relatórios consideram apenas `status = 'paid'` por padrão. Use `--include-pending` (relatórios) ou `--projected` (saldo) para incluir previstos.
- **Transferências:** saldo das contas inclui transferências; relatórios de receita/despesa **excluem** lançamentos com `transfer_id IS NOT NULL`.
- **Cartão de crédito:** a compra é despesa na **data da compra** (competência); o pagamento da fatura é uma **transferência** da conta pagadora para o cartão (decisão D-04).
- **Idempotência:** comandos que geram dados (`recurring run`, `import csv`) podem rodar duas vezes sem duplicar.
- **Operações compostas** (transferência, parcelamento, pagamento de fatura) rodam em uma única transação SQL.

---

## Tasks

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
