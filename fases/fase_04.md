# Fase 4 — Contas avançadas

Backlog da **Fase 4 — Contas avançadas** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Tasks

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

### [x] F4-06 — Tags e referência de anexos
- **Depende de:** F2-00
- **Escopo:** tabelas `tags`, `transaction_tags` e `attachments` (transação, URI, SHA-256 opcional, nota). Comandos `tag add|list|rm`, `tx tag <id> <tags...>`, `tx list --tag`, `tx attach <id> <caminho|url>` (só guarda a referência; não armazena o arquivo).
- **Critérios de aceite:**
  - Anexo local tem a existência verificada e o hash calculado
  - `tag rm` pede confirmação quando a tag está em uso
  - `report categories --tag` filtra por tag, se F2-03 estiver concluída
- **Notas:** Implementado gerenciamento de tags (`TagService`, `TagRepository`, `finctl tag add|list|rm`) com verificação de uso pré-remoção e vinculação múltipla a transações (`finctl tx tag <id> <tags...>`). Implementada filtragem por tag em listagem de transações (`tx list --tag`) e em relatórios de categoria (`report categories --tag`). Suporte a anexos de referências locais com validação de existência e cálculo do hash SHA-256 ou URLs remotas (`finctl tx attach <id> <uri> [--note]`). Testes de integração validam todo o ciclo de criação, consulta, anexos e deleção segura.

---

## Gate — Revisão das Fases 2–4

### [x] G-01 — Revisão e documentação
- **Depende de:** todas as tasks das Fases 2–4
- **Escopo:** teste de ponta a ponta (conta → lançamentos → recorrência → parcelas → cartão → pagamento de fatura → relatórios), README atualizado com os novos comandos e revisão das regras transversais.
- **Critérios de aceite:**
  - Cenário E2E automatizado passa no CI
  - Relatórios nunca contam transferências nem lançamentos `pending` por padrão
  - README cobre todos os comandos novos
- **Notas:** Implementado teste de ciclo completo de ponta a ponta (`crates/storage/tests/e2e_full_lifecycle_test.rs`) cobrindo configuração de contas bancárias e cartão de crédito, hierarquia de categorias, execução e idempotência de regras recorrentes, lançamentos previstos vs realizados, parcelamento com distribuição em faturas consecutivas, rollovers após data de fechamento, conciliação e pagamento integral de fatura com restauração de limite, transferências entre contas, anexação de comprovantes com hash SHA-256 e tags, monitoramento e alerta de orçamentos, isolamento contábil e consistência dos relatórios mensais, anuais e por categoria. `README.md` completamente reformulado e detalhado com guia de início rápido, formatos de saída (`table|json|csv`), códigos de saída (`0`, `1`, `2`), documentação de regras transversais e exemplos práticos para todos os comandos das Fases 1 a 4. Todos os testes do workspace e verificações de qualidade (`cargo fmt`, `cargo clippy`, `cargo test`) passaram com sucesso.
