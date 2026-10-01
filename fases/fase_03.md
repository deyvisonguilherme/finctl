# Fase 3 — Planejamento

Backlog da **Fase 3 — Planejamento** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Tasks

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
