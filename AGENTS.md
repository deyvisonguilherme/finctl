# AGENTS.md — finctl

Sistema de controle financeiro pessoal (receitas e despesas) em **Rust (CLI) + PostgreSQL**.
Este arquivo orienta agentes de IA (Antigravity CLI / `agy`) e colaboradores humanos.

## Como trabalhar neste repositório (GitFlow)

1. Leia `TASKS.md` e escolha **uma** task com status `[ ]` cujas dependências estejam concluídas.
2. Certifique-se de estar na branch `develop` atualizada (`git checkout develop && git pull origin develop`).
3. Crie a branch de feature a partir de `develop`: `feat/<id-da-task>-<slug>` ou `feature/<id-da-task>-<slug>` (ex.: `feat/F2-01-monthly-summary`).
4. Marque a task como `[~]` (em andamento) em `TASKS.md` antes de começar.
5. Implemente apenas o escopo da task. Se achar algo fora do escopo, registre em "Backlog / Ideias" no `TASKS.md` e siga em frente.
6. Antes de concluir, rode o checklist de qualidade (abaixo).
7. Faça o commit seguindo Conventional Commits, mergeie a branch de feature na `develop` e remova a branch de feature local/remota se aplicável.
8. Marque a task como `[x]`, preencha a linha de **Notas** se houver decisão relevante e faça o commit.
9. Releases e Hotfixes seguem o fluxo padrão GitFlow (`release/vX.Y.Z` ou `hotfix/vX.Y.Z` mergeados em `main` com tag e em `develop`).

Nunca trabalhe em duas tasks ao mesmo tempo, a menos que sejam independentes e solicitadas explicitamente.

## Checklist de qualidade (obrigatório antes de marcar `[x]`)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Se a task alterar o banco, rode também os testes de integração (exigem Docker/Podman para o Postgres de teste) e confirme que as migrations aplicam do zero.

## Stack

| Área | Escolha |
|---|---|
| Linguagem | Rust (edição 2021 ou superior) |
| CLI | `clap` (derive) |
| Saída em terminal | `comfy-table` (tabelas), `dialoguer` (prompts) |
| Banco | PostgreSQL + `sqlx` (migrations em `crates/storage/migrations`) |
| Dinheiro | `rust_decimal` ↔ `NUMERIC(14,2)` |
| Datas | `chrono` |
| Erros | `thiserror` (libs) e `anyhow` (binário) |
| Logs | `tracing` |
| Testes de integração | `testcontainers` |

## Estrutura do workspace

```
finctl/
├── Cargo.toml              # workspace
├── AGENTS.md
├── TASKS.md
├── docker-compose.yml      # Postgres de desenvolvimento
├── crates/
│   ├── domain/             # entidades, value objects, regras (sem I/O, sem sqlx)
│   ├── storage/            # repositórios sqlx + migrations
│   ├── app/                # casos de uso (orquestram domain + storage)
│   └── cli/                # binário `finctl`: clap, parsing, formatação de saída
└── docs/
    └── specs/              # specs curtas por feature
```

**Regra de dependência:** `cli → app → (domain, storage)`; `storage → domain`; `domain` não depende de ninguém.
O `domain` não conhece `sqlx`, `clap` nem I/O.

## Convenções de código

- **Dinheiro:** sempre `rust_decimal::Decimal` no código e `NUMERIC(14,2)` no banco. **Nunca `f64`/`f32`.**
- **Valor sempre positivo** + campo `tipo` (`income` | `expense`). O sinal é derivado do tipo, nunca armazenado.
- **Datas de lançamento:** `DATE`. **Auditoria** (`created_at`, `updated_at`): `TIMESTAMPTZ`.
- **Erros:** nada de `unwrap()`/`expect()` fora de testes. Use `?` e erros tipados no `domain`/`app`.
- **SQL:** prefira `sqlx::query!`/`query_as!` (verificação em compile-time). Para builds offline, mantenha `.sqlx/` atualizado (`cargo sqlx prepare --workspace`).
- **Nomes:** código e identificadores em inglês; mensagens ao usuário, `--help` e documentação em português (pt-BR).
- **Commits:** Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`), com o ID da task no escopo quando houver (`feat(F1-03): adiciona finctl income add`).

## Banco de dados e migrations

- Uma migration por mudança, nomeada de forma descritiva (`sqlx migrate add create_accounts`).
- **Nunca edite uma migration já aplicada/commitada.** Corrija com uma nova migration.
- Toda migration deve aplicar do zero em um banco vazio.
- Índices esperados: `transactions(account_id, date)` e `transactions(category_id, date)`.
- Operações compostas (transferências, parcelamentos) devem rodar dentro de **uma transação SQL**.

## CLI: padrões de UX

- Subcomandos no formato `finctl <recurso> <ação>` (ex.: `finctl account add`).
- Todo comando de listagem aceita `--format table|json|csv` (padrão: `table`).
- Códigos de saída: `0` sucesso, `1` erro de uso/validação, `2` erro de infraestrutura (banco, I/O).
- Mensagens de erro claras e acionáveis, sem stack trace por padrão (`RUST_LOG`/`--verbose` para detalhes).
- Valores monetários exibidos no formato brasileiro (`R$ 1.234,56`) na saída `table`; `json`/`csv` usam ponto decimal sem símbolo.

## Configuração

- Variáveis em `.env` (nunca commitar). Mantenha `.env.example` atualizado.
- `DATABASE_URL` é obrigatória. Demais opções via flags ou arquivo de config.

## O que o agente NÃO deve fazer

- Não executar comandos destrutivos no banco (`DROP`, `TRUNCATE`, `DELETE` sem `WHERE`) fora do banco de testes/containers descartáveis.
- Não commitar segredos, `.env` ou dumps com dados reais.
- Não adicionar dependências sem justificar na mensagem de commit (prefira o que já está na stack).
- Não alterar a estrutura do workspace ou as convenções deste arquivo sem pedir confirmação.
- Não marcar uma task como concluída com testes falhando ou `clippy` com warnings.

## Definição de pronto (DoD)

Uma task está pronta quando:

- [ ] Os critérios de aceite da task em `TASKS.md` foram atendidos
- [ ] O checklist de qualidade passa
- [ ] Há testes cobrindo o comportamento novo (unitário no `domain`, integração no `storage`/CLI quando aplicável)
- [ ] `--help` do comando novo está em português e descreve as flags
- [ ] `TASKS.md` foi atualizado

## Decisões de design em aberto

Se uma task depender de uma destas decisões, siga o padrão sugerido e anote em **Notas**; se for estrutural, pergunte antes.

| ID | Decisão | Padrão sugerido |
|---|---|---|
| D-01 | Uso pessoal ou multiusuário | Incluir `user_id` desde a Fase 0 para evitar migration dolorosa depois |
| D-02 | Saldo calculado ou armazenado | Calculado via `SUM`; otimizar só se ficar lento |
| D-03 | Valor com sinal ou campo `tipo` | Valor positivo + `tipo` |
