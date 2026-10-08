# finctl — Sistema de Controle Financeiro Pessoal (CLI)

`finctl` é uma ferramenta de linha de comando robusta, desenvolvida em **Rust** e **PostgreSQL**, para controle financeiro pessoal completo. Permite o gerenciamento de contas, cartões de crédito com faturas, compras parceladas, transações recorrentes, orçamentos, conciliação bancária, relatórios analíticos e controle de fluxo de caixa (realizado vs. previsto).

---

## 🚀 Começando Rápido

### 1. Pré-requisitos
- [Rust](https://www.rust-lang.org/) (edição 2021 ou superior)
- [Docker](https://www.docker.com/) ou [Podman](https://podman.io/)

### 2. Configurar o ambiente e subir o banco de dados
Copie o arquivo de exemplo de ambiente e inicie o container do PostgreSQL:

```bash
cp .env.example .env
docker compose up -d
# ou com Podman:
# podman compose up -d
```

### 3. Inicializar o finctl
Execute o comando `init` para aplicar automaticamente todas as migrações do banco de dados e semear as categorias padrão essenciais:

```bash
finctl init
```

Para inicializar apenas aplicando as migrações sem semear categorias iniciais:
```bash
finctl init --no-seed
```

---

## 🎯 Padrões de Uso, Formatos e Códigos de Saída

### Formatos de Saída
Todos os comandos de listagem e relatórios suportam o argumento `--format`:
- `table` (padrão): Exibição tabular amigável no terminal, com valores em moeda brasileira (`R$ 1.234,56`) e destaques visuais.
- `json`: Saída estruturada em JSON com valores decimais puros para automação e integração.
- `csv`: Saída em valores separados por vírgula.

### Códigos de Saída (Exit Codes)
- `0`: Sucesso na operação.
- `1`: Erro de validação de regras de negócio ou uso incorreto de argumentos.
- `2`: Erro de infraestrutura (falha de conexão ao banco de dados, variáveis ausentes, erro de I/O de arquivos).

---

## ⚖️ Regras Transversais de Negócio

- **Realizado × Previsto:** Consultas de saldo (`finctl balance`) e relatórios analíticos (`finctl report`) consideram **apenas** transações com status `paid` (realizadas) por padrão. Use `--projected` no saldo ou `--include-pending` nos relatórios para incluir lançamentos previstos (`pending`).
- **Transferências Contábeis:** Transferências entre contas alteram o saldo das respectivas contas, mas são **excluídas** dos relatórios de receitas e despesas.
- **Cartão de Crédito:** As compras no cartão são registradas como despesas na **data da compra** (regime de competência). O pagamento da fatura (`finctl card pay`) é tratado contabilmente como uma **transferência** entre a conta pagadora e a conta do cartão.
- **Idempotência:** Comandos geradores de dados (`finctl recurring run`, `finctl import csv`) podem ser executados múltiplas vezes sem duplicar transações.
- **Atomicidade:** Operações compostas (transferências entre contas, parcelamentos em lote, pagamentos de fatura) são executadas dentro de uma única transação SQL atômica.

---

## 📖 Guia Completo de Comandos

### 1. Banco de Dados (`finctl db`)
- **Verificar conectividade com o banco:**
  ```bash
  finctl db ping
  ```
- **Executar migrações pendentes:**
  ```bash
  finctl db migrate
  ```

---

### 2. Contas Bancárias e Cartões (`finctl account`)
- **Criar contas bancárias ou carteiras:**
  ```bash
  finctl account add "Nubank" --kind checking --initial-balance 1500,00
  finctl account add "Carteira" --kind wallet --initial-balance 120,50
  finctl account add "Reserva de Emergência" --kind savings --initial-balance 10000,00
  finctl account add "XP Investimentos" --kind investment --initial-balance 25000,00
  ```
- **Criar conta de Cartão de Crédito:**
  ```bash
  finctl account add "Cartão Nubank" --kind credit_card --closing-day 25 --due-day 5 --credit-limit 5000,00
  ```
- **Listar todas as contas:**
  ```bash
  finctl account list
  finctl account list --format json
  ```

---

### 3. Categorias (`finctl category`)
- **Criar categorias e subcategorias:**
  ```bash
  finctl category add "Alimentação" --kind expense
  finctl category add "Supermercado" --kind expense --parent "Alimentação"
  finctl category add "Restaurante" --kind expense --parent "Alimentação"
  finctl category add "Salário" --kind income
  ```
- **Listar árvore de categorias:**
  ```bash
  finctl category list
  ```

---

### 4. Lançamentos (`finctl income` / `finctl expense` / `finctl tx`)
- **Registrar receitas e despesas avulsas:**
  ```bash
  finctl income add --account "Nubank" --category "Salário" --amount 5000,00 --desc "Salário Mensal"
  finctl expense add --account "Nubank" --category "Supermercado" --amount 250,80 --date 2026-10-02 --desc "Compras do mês"
  ```
- **Registrar lançamentos previstos (pendentes):**
  ```bash
  finctl expense add --account "Nubank" --category "Moradia" --amount 1200,00 --date 2026-10-10 --desc "Aluguel" --pending
  ```
- **Registrar compras parceladas:**
  ```bash
  # Divide R$ 1.200,00 em 6 parcelas mensais:
  finctl expense add --account "Cartão Nubank" --category "Eletrônicos" --amount 1200,00 --installments 6 --desc "Notebook"

  # Ou especificando o valor de cada parcela:
  finctl expense add --account "Cartão Nubank" --category "Cursos" --installment-amount 150,00 --installments 4 --desc "Curso Rust"
  ```
- **Confirmar pagamento de lançamento pendente:**
  ```bash
  finctl tx pay <ID_DA_TRANSACAO>
  # ou informando a data efetiva do pagamento:
  finctl tx pay <ID_DA_TRANSACAO> --date 2026-10-10
  ```
- **Consultar lançamentos com filtros avançados:**
  ```bash
  finctl tx list
  finctl tx list --month 2026-10
  finctl tx list --from 2026-10-01 --to 2026-10-15 --account "Nubank" --kind expense
  finctl tx list --status pending
  finctl tx list --tag "essencial"
  finctl tx list --group <INSTALLMENT_GROUP_ID>
  ```
- **Editar ou remover lançamentos:**
  ```bash
  finctl tx edit <ID> --amount 280,00 --desc "Supermercado e padaria"
  finctl tx rm <ID>
  # Remoção ou edição em lote de parcelas pendentes:
  finctl tx rm --group <INSTALLMENT_GROUP_ID> --yes
  ```

---

### 5. Transferências entre Contas (`finctl transfer`)
- **Transferir valores entre contas:**
  ```bash
  finctl transfer add --from "Nubank" --to "Reserva de Emergência" --amount 500,00 --desc "Aporte mensal"
  ```

---

### 6. Cartão de Crédito e Faturas (`finctl card`)
- **Listar e detalhar faturas do cartão:**
  ```bash
  finctl card invoice list "Cartão Nubank"
  finctl card invoice show "Cartão Nubank" --month 2026-10
  ```
- **Fechar fatura:**
  ```bash
  finctl card invoice close "Cartão Nubank" --month 2026-10
  ```
- **Pagar fatura (total ou parcial):**
  ```bash
  # Pagamento integral da fatura atual:
  finctl card pay "Cartão Nubank" --from "Nubank" --month 2026-10

  # Pagamento parcial:
  finctl card pay "Cartão Nubank" --from "Nubank" --month 2026-10 --amount 500,00
  ```

---

### 7. Orçamentos por Categoria (`finctl budget`)
- **Definir orçamento mensal:**
  ```bash
  # Orçamento recorrente para a categoria:
  finctl budget set "Alimentação" 1500,00

  # Exceção pontual para um mês específico:
  finctl budget set "Lazer" 800,00 --month 2026-12
  ```
- **Acompanhar execução orçamentária:**
  ```bash
  finctl budget status
  finctl budget status --month 2026-10
  ```
  *Exibe valor orçado, consumido (incluindo subcategorias), saldo restante e status (`OK`, `ALERTA ≥ 80%`, `ESTOURADO`). Ao registrar uma despesa que ultrapasse o orçamento, um aviso é emitido no terminal.*

---

### 8. Lançamentos Recorrentes (`finctl recurring`)
- **Criar regras de recorrência:**
  ```bash
  finctl recurring add --account "Nubank" --category "Salário" --kind income --amount 6000,00 --freq monthly --day 5 --desc "Salário Mensal"
  finctl recurring add --account "Nubank" --category "Moradia" --kind expense --amount 1200,00 --freq monthly --day 10 --desc "Aluguel"
  ```
- **Gerenciar regras:**
  ```bash
  finctl recurring list
  finctl recurring pause <ID_DA_REGRA>
  finctl recurring resume <ID_DA_REGRA>
  finctl recurring edit <ID_DA_REGRA> --amount 1300,00
  finctl recurring rm <ID_DA_REGRA>
  ```
- **Processar e gerar lançamentos:**
  ```bash
  finctl recurring run
  # Simulação sem gravar no banco:
  finctl recurring run --dry-run
  # Gerar ocorrências até uma data limite:
  finctl recurring run --until 2026-12-31
  ```

#### Agendamento Automático de Recorrências
- **Via Cron (`crontab -e`):**
  ```cron
  0 6 * * * finctl recurring run >> /var/log/finctl-recurring.log 2>&1
  ```
- **Via Systemd Timer (`~/.config/systemd/user/finctl-recurring.timer`):**
  ```ini
  [Timer]
  OnCalendar=daily
  Persistent=true
  ```

---

### 9. Relatórios Analíticos (`finctl report`)
- **Relatório Mensal e Evolução:**
  ```bash
  finctl report monthly
  finctl report monthly --month 2026-10
  finctl report monthly --year 2026
  finctl report monthly --include-pending
  ```
  *Exibe total de receitas, despesas, saldo líquido e taxa de poupança (Savings Rate).*

- **Relatório por Categorias:**
  ```bash
  finctl report categories --month 2026-10
  finctl report categories --kind expense --depth 1
  finctl report categories --tag "essencial"
  ```
  *Exibe distribuição percentual e total ordenado do maior para o menor.*

- **Comparativo Temporal:**
  ```bash
  finctl report compare --months 2026-08,2026-09,2026-10
  finctl report compare --last 3
  ```
  *Exibe evolução de gastos mês a mês com variações absolutas e percentuais.*

---

### 10. Importação, Exportação e Conciliação (`finctl export` / `finctl import` / `finctl reconcile`)
- **Exportar lançamentos:**
  ```bash
  finctl export tx --output extrato.csv --format csv --month 2026-10
  finctl export tx --output relatorio.json --format json
  ```
  *O CSV padrão pt-BR utiliza `;` como separador, vírgula decimal e BOM UTF-8 compatível diretamente com o Excel.*

- **Importar extrato CSV:**
  ```bash
  # Simulação (dry-run):
  finctl import csv extrato_nubank.csv --account "Nubank" --dry-run

  # Importação real com perfil:
  finctl import csv extrato_nubank.csv --account "Nubank" --profile nubank
  ```
  *Utiliza hash SHA-256 (`import_hash`) para garantir idempotência completa e envia transações sem correspondência para a categoria de sistema "A classificar".*

- **Conciliação bancária:**
  ```bash
  finctl reconcile --account "Nubank" --file extrato.csv
  finctl reconcile status --account "Nubank"
  ```
  *Analisa similaridade de descrição, data (tolerância de ±3 dias) e valor, apresentando proposta para confirmação e gravando `reconciled_at`.*

---

### 11. Tags e Anexos (`finctl tag` / `finctl tx tag` / `finctl tx attach`)
- **Gerenciar Tags:**
  ```bash
  finctl tag add "viagem-ferias"
  finctl tag list
  finctl tag rm "viagem-ferias"
  ```
- **Vincular tags a transações:**
  ```bash
  finctl tx tag <ID_TRANSACAO> viagem-ferias hotel
  ```
- **Anexar comprovantes e referências:**
  ```bash
  # Arquivo local (com verificação de integridade via hash SHA-256):
  finctl tx attach <ID_TRANSACAO> /caminho/para/recibo.pdf --note "Recibo de pagamento"

  # Link web ou documento na nuvem:
  finctl tx attach <ID_TRANSACAO> https://drive.google.com/file/d/xyz/view
  ```

---

### 12. Consulta de Saldos (`finctl balance`)
- **Saldo consolidado de todas as contas:**
  ```bash
  finctl balance
  ```
- **Saldo projetado (inclui lançamentos pendentes):**
  ```bash
  finctl balance --projected
  ```
- **Saldo retroativo em data específica:**
  ```bash
  finctl balance --at 2026-10-01
  finctl balance --format json
  ```

---

### 13. Interface Interativa em Terminal (`finctl tui`)

O `finctl` oferece uma interface rica baseada em texto (**TUI**) desenvolvida com `ratatui` e `crossterm`, utilizando arquitetura estilo Elm (`Model`, `Message`, `update`, `view`) com consultas de banco assíncronas em segundo plano.

- **Iniciar a TUI:**
  ```bash
  # Iniciar com o tema escuro padrão:
  finctl tui

  # Iniciar especificando tema claro:
  finctl tui --theme light
  ```

#### Mapa Completo de Teclas e Atalhos

| Contexto | Tecla(s) | Ação / Descrição |
|---|---|---|
| **Geral & Navegação** | `?` | Abrir / fechar painel de ajuda |
| | `t` | Alternar tema visual dinamicamente (Claro / Escuro) |
| | `q` ou `Ctrl+C` | Encerrar a aplicação TUI |
| | `Tab` / `BackTab` | Avançar / retroceder aba ativa |
| | `1` a `5` | Acesso direto às abas (`1: Dash`, `2: Lanç.`, `3: Relat.`, `4: Orçam.`, `5: Metas`) |
| | `r` | Recarregar dados do banco de dados |
| **Dashboard** | `r` | Atualizar indicadores e resumos do mês |
| **Lançamentos (Tabela)** | `j` / `↓` | Mover cursor para o lançamento abaixo |
| | `k` / `↑` | Mover cursor para o lançamento acima |
| | `Espaço` | Alternar seleção do item (suporte a seleção múltipla) |
| | `[` / `PgUp` | Página anterior de lançamentos |
| | `]` / `PgDn` | Próxima página de lançamentos |
| | `a` | Abrir formulário para adicionar novo lançamento |
| | `e` ou `Enter` | Editar lançamento sob o cursor |
| | `d` | Excluir lançamento(s) selecionado(s) com confirmação |
| | `p` | Marcar lançamento(s) selecionado(s) como pago |
| | `/` | Iniciar busca rápida textual por descrição |
| | `f` | Abrir modal com filtros avançados (mês, conta, categoria, tipo, status, tag) |
| **Modais & Formulários** | `Tab` / `Enter` | Avançar para o próximo campo |
| | `Shift+Tab` / `↑` | Retroceder para o campo anterior |
| | `Espaço` | Alternar valor em campos de múltipla escolha (Tipo, Status) |
| | `Esc` | Cancelar edição / fechar diálogo |
| **Relatórios** | `1` | Subvisão: Gastos por Categorias (com barras Unicode) |
| | `2` | Subvisão: Evolução Mensal (Sparklines de receitas e despesas) |
| | `3` | Subvisão: Comparativo detalhado com mês anterior |
| | `[` | Mês de referência anterior |
| | `]` | Próximo mês de referência |
| | `i` | Alternar inclusão de lançamentos previstos (`pending`) |
| | `p` | Definir mês de referência customizado (`AAAA-MM`) |
| | `j` / `k` | Rolar tabela de categorias ou comparativo |
| **Painel de Ajuda** | `j` / `↓` | Rolar texto de ajuda para baixo |
| | `k` / `↑` | Rolar texto de ajuda para cima |
| | `?` ou `Esc` | Fechar o painel de ajuda |

---


## 🛠️ Arquitetura e Qualidade

O projeto adota Clean Architecture em camadas com responsabilidades estritamente separadas:
- `crates/domain`: Entidades puras, Value Objects (`Money`), validações de domínio (zero I/O ou dependências externas de banco).
- `crates/storage`: Repositórios PostgreSQL com `sqlx`, migrações e testes de integração isolados via `testcontainers`.
- `crates/app`: Casos de uso e orquestração de serviços de negócio.
- `crates/cli`: Interface CLI com `clap` (derive), tabelas com `comfy-table` e diálogos com `dialoguer`.

### Checklist de Qualidade Obrigatório
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
