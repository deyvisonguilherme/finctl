# finctl — Sistema de Controle Financeiro Pessoal (CLI)

`finctl` é uma ferramenta de linha de comando desenvolvida em **Rust** e **PostgreSQL** para controle financeiro pessoal, permitindo o gerenciamento de contas, categorias de receitas e despesas, lançamentos financeiros e visualização de saldos consolidados.

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
# ou com podman:
# podman compose up -d
```

### 3. Inicializar o finctl
Execute o comando `init` para aplicar todas as migrações do banco e semear as categorias padrão de receitas e despesas:

```bash
cargo run -- init
```

Para inicializar sem semear as categorias automáticas:
```bash
cargo run -- init --no-seed
```

---

## 📖 Guia de Uso dos Comandos

### Banco de Dados (`finctl db`)
- **Verificar conexão:**
  ```bash
  finctl db ping
  ```
- **Aplicar migrações:**
  ```bash
  finctl db migrate
  ```

---

### Contas Bancárias e Carteiras (`finctl account`)
- **Criar uma conta:**
  ```bash
  finctl account add "Nubank" --kind checking --initial-balance 1500,00
  finctl account add "Carteira" --kind wallet --initial-balance 100,00
  finctl account add "Reserva de Emergência" --kind savings --initial-balance 5000,00
  ```
  *Tipos válidos:* `checking` (corrente), `savings` (poupança), `wallet` (carteira), `investment` (investimento).

- **Listar contas:**
  ```bash
  finctl account list
  finctl account list --format json
  finctl account list --format csv
  ```

---

### Categorias (`finctl category`)
- **Criar uma categoria raiz:**
  ```bash
  finctl category add "Alimentação" --kind expense
  finctl category add "Salário" --kind income
  ```
- **Criar uma subcategoria:**
  ```bash
  finctl category add "Supermercado" --kind expense --parent "Alimentação"
  finctl category add "Restaurante" --kind expense --parent "Alimentação"
  ```
- **Listar categorias com hierarquia:**
  ```bash
  finctl category list
  finctl category list --format json
  ```

---

### Lançamentos de Receitas e Despesas (`finctl income` / `finctl expense`)
- **Registrar uma receita:**
  ```bash
  finctl income add --account "Nubank" --category "Salário" --amount 4500,00 --desc "Salário do mês"
  ```
- **Registrar uma despesa:**
  ```bash
  finctl expense add --account "Nubank" --category "Supermercado" --amount 189,50 --date 2026-10-01 --desc "Compras do mês"
  ```

---

### Consulta e Gerenciamento de Transações (`finctl tx`)
- **Listar lançamentos recentes (padrão: últimos 30 dias):**
  ```bash
  finctl tx list
  ```
- **Filtrar por mês ou período:**
  ```bash
  finctl tx list --month 2026-10
  finctl tx list --from 2026-10-01 --to 2026-10-15
  ```
- **Filtrar por conta, categoria e tipo:**
  ```bash
  finctl tx list --account "Nubank" --kind expense
  finctl tx list --category "Supermercado" --limit 10
  finctl tx list --format json
  ```
- **Editar um lançamento existente:**
  ```bash
  finctl tx edit <ID> --amount 195,00 --desc "Compras com itens extras"
  ```
- **Remover um lançamento:**
  ```bash
  finctl tx rm <ID>
  # ou pular a confirmação interativa:
  finctl tx rm <ID> --yes
  ```

---

### Saldos Consolidados (`finctl balance`)
- **Exibir saldo consolidado por conta e total geral:**
  ```bash
  finctl balance
  ```
- **Exibir saldo retroativo até uma data específica:**
  ```bash
  finctl balance --at 2026-10-05
  finctl balance --format json
  ```

---

## 🛠️ Arquitetura e Qualidade

O projeto segue arquitetura em camadas e convenções estritas descritas em `AGENTS.md`:
- `crates/domain`: Entidades, Value Objects (`Money`), validações de negócio puras (sem I/O ou SQL).
- `crates/storage`: Repositórios PostgreSQL (`sqlx`), migrations e helpers de teste com `testcontainers`.
- `crates/app`: Casos de uso e orquestração de serviços de negócio.
- `crates/cli`: Interface de linha de comando (`clap`), formatação de tabelas (`comfy-table`) e prompts (`dialoguer`).

### Executando os testes e checagens de qualidade
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
