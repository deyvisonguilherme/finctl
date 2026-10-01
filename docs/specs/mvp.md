# Especificação do MVP (Fases 0 e 1)

## Escopo Implementado

### 1. Fundação (Fase 0)
- Workspace Cargo (`domain`, `storage`, `app`, `cli`)
- Docker Compose com PostgreSQL 16 Alpine
- Pool de conexões sqlx e tratamento de erros tipados
- Value Object `Money` com `rust_decimal` e formatação `R$ 1.234,56`
- Schema e Migrations com integridade referencial, índices e constraints
- Infraestrutura de testes de integração com `testcontainers` e isolamento por banco/container

### 2. MVP Funcional (Fase 1)
- `finctl init [--no-seed]`: Inicialização de banco e seed idempotente de categorias padrão
- `finctl db ping` e `finctl db migrate`
- `finctl account add` e `finctl account list` (com formatos `table`, `json`, `csv`)
- `finctl category add` e `finctl category list` (hierarquia até 2 níveis, exibição em árvore)
- `finctl income add` e `finctl expense add` (validação de categoria compatível com o tipo)
- `finctl tx list` (filtros combináveis por `--from`, `--to`, `--month`, `--account`, `--category`, `--kind`, `--limit`)
- `finctl tx edit` e `finctl tx rm` (confirmação interativa com dialoguer ou flag `--yes`)
- `finctl balance` (saldo por conta calculado via `initial_balance + sum(income) - sum(expense)`, filtro `--at <data>`, linha de TOTAL GERAL)
