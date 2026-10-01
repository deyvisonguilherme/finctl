# Fase 1 — MVP: receitas e despesas

Backlog da **Fase 1 — MVP: receitas e despesas** do `finctl`. Regras gerais de contribuição estão em `AGENTS.md` e o índice geral em `TASKS.md`.

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## Tasks

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
