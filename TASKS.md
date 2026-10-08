# TASKS.md — finctl

Backlog geral e índice das fases de desenvolvimento do projeto `finctl`.
As regras e instruções operacionais para colaboradores e agentes de IA estão definidas em [`AGENTS.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/AGENTS.md).

**Legenda de status:** `[ ]` pendente · `[~]` em andamento · `[x]` concluída · `[!]` bloqueada

---

## 🗺️ Índice de Fases e Status

| Fase | Arquivo | Escopo Principal | Tasks Concluídas | Status |
|---|---|---|---|---|
| **Fase 0** | [`fases/fase_00.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_00.md) | Fundação, workspace Cargo, Docker, PostgreSQL, Money e CI | 6 / 6 | `[x]` Concluída |
| **Fase 1** | [`fases/fase_01.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_01.md) | MVP: contas, categorias, receitas, despesas, saldos e init | 8 / 8 | `[x]` Concluída |
| **Fase 2** | [`fases/fase_02.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_02.md) | Relatórios mensais, por categoria, comparativos e import/export CSV | 7 / 7 | `[x]` Concluída |
| **Fase 3** | [`fases/fase_03.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_03.md) | Planejamento: status previsto × realizado, orçamentos, recorrências e parcelas | 5 / 5 | `[x]` Concluída |
| **Fase 4** | [`fases/fase_04.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_04.md) | Contas avançadas: transferências, cartão de crédito, faturas, conciliação, tags/anexos e Gate G-01 | 7 / 7 | `[x]` Concluída |
| **Fase 5** | [`fases/fase_05.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_05.md) | Robustez e release: soft delete, auditoria, backup/restore, completions, multi-arch e Gate G-02 | 3 / 7 | `[~]` Em andamento |
| **Fase 6** | [`fases/fase_06.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/fase_06.md) | Evoluções: TUI com `ratatui`, Metas de economia, Projeção de fluxo de caixa e Gate G-03 | 0 / 10 | `[ ]` Pendente |
| **Backlog** | [`fases/backlog.md`](file:///home/deyvison/Documentos/Projects/Learn/finctl/fases/backlog.md) | Ideias futuras sem prioridade definida (API HTTP, multi-moeda, webhooks) | — | `[ ]` Backlog |

---

## 🧭 Como Navegar e Trabalhar nas Tasks

1. **Escolha da Task:** Consulte a fase ativa no índice acima, abra o arquivo correspondente em `fases/` e escolha uma task com status `[ ]` cujas dependências estejam concluídas.
2. **Branch de Trabalho:** Crie a branch `feat/<id-da-task>-<slug>` (exemplo: `feat/F5-01-soft-delete`).
3. **Atualização de Status:** Marque a task como `[~]` (em andamento) no respectivo arquivo da fase em `fases/`.
4. **Qualidade:** Execute o checklist obrigatório (`cargo fmt`, `cargo clippy`, `cargo test`) antes de finalizar.
5. **Conclusão:** Atualize o status para `[x]`, preencha as notas e atualize a tabela deste índice caso a fase seja finalizada.
