/// Categorias de atalhos da interface TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutCategory {
    Global,
    Dashboard,
    Transactions,
    TransactionModals,
    Reports,
    Help,
}

impl ShortcutCategory {
    pub const ALL: [ShortcutCategory; 6] = [
        ShortcutCategory::Global,
        ShortcutCategory::Dashboard,
        ShortcutCategory::Transactions,
        ShortcutCategory::TransactionModals,
        ShortcutCategory::Reports,
        ShortcutCategory::Help,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Global => "Geral & Navegação",
            Self::Dashboard => "Dashboard",
            Self::Transactions => "Lançamentos (Tabela)",
            Self::TransactionModals => "Formulários & Modais",
            Self::Reports => "Relatórios Analíticos",
            Self::Help => "Painel de Ajuda",
        }
    }
}

/// Representa um atalho de teclado registrado na TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub key: &'static str,
    pub description: &'static str,
    pub category: ShortcutCategory,
}

/// Registro central de atalhos da aplicação.
pub struct ShortcutRegistry;

impl ShortcutRegistry {
    /// Retorna todos os atalhos disponíveis no sistema.
    pub fn all() -> Vec<Shortcut> {
        vec![
            // Geral & Navegação
            Shortcut {
                key: "?",
                description: "Abrir / fechar este painel de ajuda",
                category: ShortcutCategory::Global,
            },
            Shortcut {
                key: "t",
                description: "Alternar tema visual (Claro / Escuro)",
                category: ShortcutCategory::Global,
            },
            Shortcut {
                key: "q / Ctrl+C",
                description: "Encerrar a aplicação TUI",
                category: ShortcutCategory::Global,
            },
            Shortcut {
                key: "Tab / BackTab",
                description: "Avançar / retroceder aba ativa",
                category: ShortcutCategory::Global,
            },
            Shortcut {
                key: "1 - 5",
                description:
                    "Acesso direto às abas (1: Dash, 2: Lanç., 3: Relat., 4: Orçam., 5: Metas)",
                category: ShortcutCategory::Global,
            },
            Shortcut {
                key: "r",
                description: "Recarregar dados do banco de dados",
                category: ShortcutCategory::Global,
            },
            // Dashboard
            Shortcut {
                key: "r",
                description: "Atualizar indicadores e resumos do mês",
                category: ShortcutCategory::Dashboard,
            },
            // Lançamentos (Tabela)
            Shortcut {
                key: "j / ↓",
                description: "Navegar para o lançamento abaixo",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "k / ↑",
                description: "Navegar para o lançamento acima",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "Space",
                description: "Alternar seleção do item (seleção múltipla)",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "[ / PgUp",
                description: "Página anterior de lançamentos",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "] / PgDn",
                description: "Próxima página de lançamentos",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "a",
                description: "Adicionar novo lançamento (receita/despesa)",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "e / Enter",
                description: "Editar lançamento sob o cursor",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "d",
                description: "Excluir lançamento(s) selecionado(s)",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "p",
                description: "Marcar lançamento(s) como pago",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "/",
                description: "Busca rápida textual por descrição",
                category: ShortcutCategory::Transactions,
            },
            Shortcut {
                key: "f",
                description: "Abrir diálogo de filtros avançados",
                category: ShortcutCategory::Transactions,
            },
            // Modais de Lançamentos
            Shortcut {
                key: "Tab / Enter",
                description: "Avançar para o próximo campo",
                category: ShortcutCategory::TransactionModals,
            },
            Shortcut {
                key: "Shift+Tab / ↑",
                description: "Retroceder para o campo anterior",
                category: ShortcutCategory::TransactionModals,
            },
            Shortcut {
                key: "Space",
                description: "Alternar opções de Tipo / Status",
                category: ShortcutCategory::TransactionModals,
            },
            Shortcut {
                key: "Esc",
                description: "Cancelar edição / fechar modal",
                category: ShortcutCategory::TransactionModals,
            },
            // Relatórios
            Shortcut {
                key: "1",
                description: "Subvisão: Gastos por Categorias",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "2",
                description: "Subvisão: Evolução Mensal (Sparklines)",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "3",
                description: "Subvisão: Comparativo com mês anterior",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "[",
                description: "Mês de referência anterior",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "]",
                description: "Próximo mês de referência",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "i",
                description: "Alternar inclusão de lançamentos previstos",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "p",
                description: "Definir mês de referência customizado (AAAA-MM)",
                category: ShortcutCategory::Reports,
            },
            Shortcut {
                key: "j / k",
                description: "Rolar lista de categorias / comparativo",
                category: ShortcutCategory::Reports,
            },
            // Painel de Ajuda
            Shortcut {
                key: "j / ↓",
                description: "Rolar texto de ajuda para baixo",
                category: ShortcutCategory::Help,
            },
            Shortcut {
                key: "k / ↑",
                description: "Rolar texto de ajuda para cima",
                category: ShortcutCategory::Help,
            },
            Shortcut {
                key: "? / Esc",
                description: "Fechar este painel de ajuda",
                category: ShortcutCategory::Help,
            },
        ]
    }

    /// Retorna atalhos filtrados por categoria.
    pub fn by_category(category: ShortcutCategory) -> Vec<Shortcut> {
        Self::all()
            .into_iter()
            .filter(|s| s.category == category)
            .collect()
    }
}
