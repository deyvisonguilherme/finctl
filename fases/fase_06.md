# Fase 6 — Evoluções selecionadas

Backlog da **Fase 6 — Evoluções selecionadas** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Visão Geral

Apenas dois itens desta fase foram detalhados: **TUI com `ratatui`** e **Metas de economia com projeção de fluxo de caixa**. O comando de início (F6-09) permite escolher entre usar a TUI ou apenas a linha de comando.

---

## Trilha A — TUI com `ratatui`

### [x] F6-01 — Crate `tui`: esqueleto e ciclo de eventos
- **Depende de:** G-02
- **Escopo:** nova crate `tui` no workspace e comando `finctl tui`. `ratatui` + `crossterm`; arquitetura estilo Elm (`Model`, `Message`, `update`, `view`); consultas ao banco rodam em *tasks* assíncronas e voltam por canal, sem travar o desenho da tela; *panic hook* que restaura o terminal.
- **Critérios de aceite:**
  - A TUI abre, redesenha ao redimensionar e fecha com `q` ou `Ctrl+C` sempre restaurando o terminal (inclusive em *panic*)
  - A crate `tui` depende apenas de `app` e `domain`, sem `sqlx`
  - Teste com `TestBackend` valida a renderização do estado inicial
- **Notas:** Criada nova crate `tui` em `crates/tui` adicionada aos members do workspace Cargo, dependendo estritamente de `domain`, `app`, `ratatui`, `crossterm`, `tokio` e `chrono` (sem nenhuma dependência de `sqlx`). Arquitetura Elm completa implementada: `Model` (estado da aplicação e navegação de abas), `Message` (eventos e ações), `Command` (intenções de background), `update` (transições de estado puras) e `view` (layout com cabeçalho, abas preliminares, conteúdo e rodapé de atalhos). Ciclo assíncrono em `run_tui` orquestrado com `tokio::select!`, `crossterm::event::EventStream`, ticker suave e canais `mpsc` bidirecionais. O comando `finctl tui` na CLI conecta o `PgPool` e dispara tarefas de background assíncronas com os serviços de `app` sem bloquear o redesenho. Panic hook com `install_panic_hook()` e restauração de terminal com `restore_terminal()` implementados. Suíte de testes com `TestBackend` cobrindo renderização da tela inicial, ciclo de vida, atalhos de teclado (`q`, `Ctrl+C`, `Tab`, `1-3`, `r`), mensagens assíncronas e encerramento gracioso em `crates/tui/tests/tui_test.rs`.


### [x] F6-02 — Dashboard
- **Depende de:** F6-01
- **Escopo:** tela inicial com saldo por conta, resumo do mês (receitas × despesas × saldo), status dos orçamentos (barras com cor por estado) e próximos vencimentos (pendentes e faturas). Navegação por abas (`Tab`/`1–5`).
- **Critérios de aceite:**
  - Os números batem com `finctl balance`, `report monthly` e `budget status` para os mesmos dados
  - Estados de carregamento e erro de banco são exibidos na tela, sem fechar a TUI
  - Funciona em terminal de 80×24
- **Notas:** Implementado Dashboard na TUI com Grid 2x2 responsivo para terminal 80x24: Quadrante 1 (Saldos por Conta e totalizador geral), Quadrante 2 (Resumo do mês corrente com receitas, despesas, saldo líquido e poupança), Quadrante 3 (Status dos orçamentos com barras de progresso coloridas por nível de alerta) e Quadrante 4 (Próximos vencimentos incluindo despesas/receitas pendentes e faturas de cartão de crédito não pagas com indicador de atraso). Criado o serviço `DashboardService` em `app` agregando `BalanceService`, `ReportService`, `BudgetService` e repositórios sem acoplar a TUI ao banco. Navegação expandida para 5 abas (`1: Dashboard`, `2: Lançamentos`, `3: Relatórios`, `4: Orçamentos`, `5: Metas`) com atalhos `Tab`, `BackTab` e teclas `1-5`. Estados de carregamento e erros de conexão tratados visualmente sem fechar a aplicação, com recarga via tecla `r`. Suíte de testes com `TestBackend` cobrindo 80x24, estados de loading/erro e navegação, além de teste de integração no storage garantindo equivalência exata dos dados com os comandos CLI.

### [x] F6-03 — Tela de lançamentos
- **Depende de:** F6-02
- **Escopo:** tabela paginada com filtros (período, conta, categoria, tipo, status, tag), busca por descrição, e formulário para adicionar, editar, remover (com confirmação) e marcar como pago, reutilizando os casos de uso de `app`.
- **Critérios de aceite:**
  - Mesmas validações da CLI (valor, categoria compatível, datas)
  - Seleção múltipla para marcar vários como pagos
  - Lista de 100 mil lançamentos navega sem travar (paginação no banco)
- **Notas:** Implementada tela completa de lançamentos na TUI com paginação eficiente no nível do PostgreSQL (`LIMIT`/`OFFSET` combinados com `COUNT` indexado) garantindo navegação instantânea em bases volumosas (>100k registros). A tabela apresenta cursor visual (`▶`), indicador de seleção (`[x]`), colunas formatadas (Data, Tipo, Conta, Categoria, Descrição, Valor em formato pt-BR e Status) ajustadas com precisão para caber em terminais 80×24 sem quebra de linha. Implementada barra superior de busca rápida (`/`) por descrição com debounce e indicador de filtros ativos (`f`), com modal interativo de filtros por mês (`AAAA-MM`), conta, categoria, tipo, status e tag. Implementados modais modais sobrepostos com `Clear` e estilização contextual para Criação/Edição (`a`/`e`/`Enter`) reutilizando `TransactionService` (com validações de valor positivo, categorias compatíveis e datas válidas), exclusão com diálogo de confirmação (`d`/`Enter`) e pagamento individual (`p`) ou em lote via seleção múltipla com barra de espaço (`Space`). Implementados comandos assíncronos no loop Elm desacoplados de I/O na TUI. Cobertura completa com 12 testes unitários/TUI com `TestBackend` e novo teste de integração PostgreSQL (`transaction_pagination_test.rs`) validando paginação, busca `ILIKE` e ações em lote.

### [ ] F6-04 — Tela de relatórios
- **Depende de:** F6-02
- **Escopo:** gráfico de barras de gastos por categoria, `Sparkline`/linha de evolução mensal de receitas e despesas e comparativo entre meses, com seletor de período e alternância entre competência e incluir previstos.
- **Critérios de aceite:**
  - Dados idênticos aos comandos `report` equivalentes
  - Legendas legíveis em 80 colunas; categorias longas são truncadas com `…`
- **Notas:**

### [ ] F6-05 — Ajuda, atalhos e testes de interface
- **Depende de:** F6-03, F6-04
- **Escopo:** painel de ajuda (`?`) com todos os atalhos, barra de status com dicas contextuais, tema claro/escuro configurável, snapshots de tela com `insta` usando `TestBackend`.
- **Critérios de aceite:**
  - Cada tela tem ao menos um teste de snapshot
  - Todos os atalhos aparecem no painel de ajuda (teste que compara a lista de atalhos registrados com a exibida)
  - A seção de README sobre a TUI inclui o mapa de teclas
- **Notas:**

### [ ] F6-09 — Comando de início: escolher TUI ou linha de comando
- **Depende de:** F6-01
- **Escopo:**
  - `finctl start [--mode tui|cli|ask]`; rodar `finctl` sem subcomando equivale a `finctl start`
  - **Modo `ask`** (padrão, D-09): em terminal interativo, mostra um menu com duas opções — `1) Interface interativa (TUI)` e `2) Linha de comando` — e a opção "lembrar minha escolha", que grava a preferência na configuração
  - **Modo `tui`:** abre a TUI direto. **Modo `cli`:** mostra o `--help` resumido com os comandos mais usados e encerra
  - Configuração em `~/.config/finctl/config.toml`, chave `ui.mode`, com os comandos `finctl config get|set|list` (ex.: `finctl config set ui.mode tui`)
  - **Precedência:** `--mode` > variável `FINCTL_MODE` > `config.toml` > padrão `ask`
  - Subcomandos explícitos (`finctl balance`, `finctl tx list`...) **nunca** passam pelo menu; `finctl tui` sempre abre a TUI, mesmo com `ui.mode = cli`
- **Critérios de aceite:**
  - Fora de um terminal interativo (pipe, cron, CI), `finctl` sem subcomando nunca pede entrada: age como modo `cli` e retorna código `0` (invocação sem comando não é erro de uso)
  - Terminal menor que 80×24 ou sem suporte à TUI: a opção da TUI é recusada com mensagem clara e o fluxo cai no modo `cli`, sem abrir uma tela quebrada
  - "Lembrar minha escolha" grava `ui.mode` e a próxima execução pula o menu; `finctl config set ui.mode ask` restaura o menu
  - Valor inválido em `--mode`, `FINCTL_MODE` ou `config.toml` retorna erro de validação (código `1`) indicando os valores aceitos
  - Testes cobrem a precedência, o menu (entrada simulada) e o comportamento sem TTY
- **Notas:**

---

## Trilha B — Metas de economia e projeção de fluxo de caixa

### [ ] F6-06 — Metas de economia
- **Depende de:** G-02
- **Escopo:** migrations `goals` (nome, valor alvo, data alvo opcional, conta vinculada opcional) e `goal_contributions` (meta, valor, data, nota). Comandos `goal add|list|show|contribute|edit|rm`. Progresso conforme D-07: saldo da conta vinculada ou soma dos aportes. `goal show` exibe percentual, valor que falta, aporte mensal necessário até a data alvo e data estimada de conclusão com base na média de aportes dos últimos 3 meses.
- **Critérios de aceite:**
  - Meta sem data alvo não exibe aporte necessário (só a estimativa)
  - Meta atingida é marcada como concluída e some do `goal list` padrão (visível com `--all`)
  - Média de aportes sem histórico suficiente mostra `n/d`, sem erro
  - Testes cobrem meta vinculada a conta e meta com aportes manuais
- **Notas:**

### [ ] F6-07 — Projeção de fluxo de caixa
- **Depende de:** G-02
- **Escopo:** `finctl forecast [--months N] [--account X] [--granularity week|month]` projeta o saldo futuro a partir do saldo atual somando: lançamentos `pending`, ocorrências futuras das regras recorrentes ainda não geradas (apenas após `last_generated_date`, sem duplicar as já materializadas), parcelas futuras e faturas de cartão no vencimento (D-08). Destaca o primeiro período em que o saldo projetado fica negativo.
- **Critérios de aceite:**
  - Nenhum valor é contado duas vezes (recorrência já gerada, parcela e compra de cartão com fatura em aberto), com teste para cada caso
  - Compra no cartão sai na data de vencimento da fatura, e não na data da compra
  - Transferências entre contas próprias não alteram o saldo total projetado
  - Suporta `--format table|json|csv`
- **Notas:**

### [ ] F6-08 — Metas e projeção na TUI
- **Depende de:** F6-02, F6-06, F6-07
- **Escopo:** tela de metas (barra de progresso, aporte necessário, registrar aporte) e tela de projeção (linha do saldo futuro com marcação do período negativo); aportes planejados de metas entram na projeção como saída opcional (`--include-goals`).
- **Critérios de aceite:**
  - Valores iguais aos de `goal show` e `forecast`
  - Com `--include-goals`, o aporte mensal necessário reduz o saldo projetado, sem contagem dupla com aportes já registrados
- **Notas:**

---

## Gate — Fase 6

### [ ] G-03 — Gate da Fase 6
- **Depende de:** F6-01 a F6-09
- **Escopo:** E2E cobrindo meta, projeção, as telas da TUI e o comando de início nos três modos; revisão de README e `CHANGELOG.md`; release minor.
- **Critérios de aceite:**
  - Cenário E2E passa no CI
  - Release publicada com o fluxo de F5-05
- **Notas:**
