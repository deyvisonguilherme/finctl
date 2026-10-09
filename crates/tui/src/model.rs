use crate::theme::{Theme, ThemeMode};
use app::{DashboardData, ReportsScreenData, TransactionDetails};
use chrono::{DateTime, Datelike, Local};
use domain::{
    CashflowForecast, ForecastGranularity, GoalId, GoalProgress, TransactionId, TransactionKind,
    TransactionStatus,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Dashboard,
    Transactions,
    Reports,
    Budgets,
    Goals,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Dashboard,
        Tab::Transactions,
        Tab::Reports,
        Tab::Budgets,
        Tab::Goals,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "1: Dashboard",
            Tab::Transactions => "2: Lançamentos",
            Tab::Reports => "3: Relatórios",
            Tab::Budgets => "4: Orçamentos",
            Tab::Goals => "5: Metas",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::Transactions => 1,
            Tab::Reports => 2,
            Tab::Budgets => 3,
            Tab::Goals => 4,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Tab::Transactions,
            2 => Tab::Reports,
            3 => Tab::Budgets,
            4 => Tab::Goals,
            _ => Tab::Dashboard,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Tab::Dashboard => Tab::Transactions,
            Tab::Transactions => Tab::Reports,
            Tab::Reports => Tab::Budgets,
            Tab::Budgets => Tab::Goals,
            Tab::Goals => Tab::Dashboard,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Tab::Dashboard => Tab::Goals,
            Tab::Transactions => Tab::Dashboard,
            Tab::Reports => Tab::Transactions,
            Tab::Budgets => Tab::Reports,
            Tab::Goals => Tab::Budgets,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    Add,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Kind,
    Account,
    Category,
    Amount,
    Date,
    Description,
    Status,
}

impl FormField {
    pub const ALL: [FormField; 7] = [
        FormField::Kind,
        FormField::Account,
        FormField::Category,
        FormField::Amount,
        FormField::Date,
        FormField::Description,
        FormField::Status,
    ];

    pub fn next(&self) -> Self {
        match self {
            Self::Kind => Self::Account,
            Self::Account => Self::Category,
            Self::Category => Self::Amount,
            Self::Amount => Self::Date,
            Self::Date => Self::Description,
            Self::Description => Self::Status,
            Self::Status => Self::Kind,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Kind => Self::Status,
            Self::Account => Self::Kind,
            Self::Category => Self::Account,
            Self::Amount => Self::Category,
            Self::Date => Self::Amount,
            Self::Description => Self::Date,
            Self::Status => Self::Description,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionFormState {
    pub mode: FormMode,
    pub editing_id: Option<TransactionId>,
    pub focused_field: FormField,
    pub kind: TransactionKind,
    pub account_input: String,
    pub category_input: String,
    pub amount_input: String,
    pub date_input: String,
    pub description_input: String,
    pub status: TransactionStatus,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterField {
    Month,
    Account,
    Category,
    Kind,
    Status,
    Tag,
}

impl FilterField {
    pub const ALL: [FilterField; 6] = [
        FilterField::Month,
        FilterField::Account,
        FilterField::Category,
        FilterField::Kind,
        FilterField::Status,
        FilterField::Tag,
    ];

    pub fn next(&self) -> Self {
        match self {
            Self::Month => Self::Account,
            Self::Account => Self::Category,
            Self::Category => Self::Kind,
            Self::Kind => Self::Status,
            Self::Status => Self::Tag,
            Self::Tag => Self::Month,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Month => Self::Tag,
            Self::Account => Self::Month,
            Self::Category => Self::Account,
            Self::Kind => Self::Category,
            Self::Status => Self::Kind,
            Self::Tag => Self::Status,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransactionFilterState {
    pub month: Option<String>,
    pub account: Option<String>,
    pub category: Option<String>,
    pub kind: Option<TransactionKind>,
    pub status: Option<TransactionStatus>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FilterModalState {
    pub focused_field: FilterField,
    pub month_input: String,
    pub account_input: String,
    pub category_input: String,
    pub kind_selection: Option<TransactionKind>,
    pub status_selection: Option<TransactionStatus>,
    pub tag_input: String,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmState {
    pub target_ids: Vec<TransactionId>,
    pub prompt_message: String,
}

#[derive(Debug, Clone)]
pub struct TransactionsTabState {
    pub items: Vec<TransactionDetails>,
    pub cursor_index: usize,
    pub selected_ids: HashSet<TransactionId>,
    pub page: i64,
    pub page_size: i64,
    pub total_count: i64,
    pub total_pages: i64,

    // Busca rápida por descrição
    pub search_query: String,
    pub is_searching: bool,

    // Filtros
    pub active_filters: TransactionFilterState,
    pub filter_modal: Option<FilterModalState>,

    // Formulário (Adicionar / Editar)
    pub form_modal: Option<TransactionFormState>,

    // Diálogo de confirmação de exclusão
    pub delete_confirm: Option<DeleteConfirmState>,
}

impl Default for TransactionsTabState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            cursor_index: 0,
            selected_ids: HashSet::new(),
            page: 1,
            page_size: 15,
            total_count: 0,
            total_pages: 1,
            search_query: String::new(),
            is_searching: false,
            active_filters: TransactionFilterState::default(),
            filter_modal: None,
            form_modal: None,
            delete_confirm: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportSubView {
    #[default]
    Categories, // [1] Gastos por Categoria
    MonthlyEvolution, // [2] Evolução Mensal (Sparklines + tabela)
    Comparison,       // [3] Comparativo entre Meses
}

impl ReportSubView {
    pub const ALL: [ReportSubView; 3] = [
        ReportSubView::Categories,
        ReportSubView::MonthlyEvolution,
        ReportSubView::Comparison,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            ReportSubView::Categories => "1: Categorias",
            ReportSubView::MonthlyEvolution => "2: Evolução Mensal",
            ReportSubView::Comparison => "3: Comparativo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodModalState {
    pub input_month: String, // e.g. "2026-10"
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportsTabState {
    pub active_subview: ReportSubView,
    pub reference_month: String, // "YYYY-MM"
    pub include_pending: bool,
    pub category_cursor: usize,
    pub comparison_cursor: usize,
    pub period_modal: Option<PeriodModalState>,
    pub data: Option<ReportsScreenData>,
}

impl Default for ReportsTabState {
    fn default() -> Self {
        let today = Local::now().date_naive();
        let reference_month = format!("{:04}-{:02}", today.year(), today.month());
        Self {
            active_subview: ReportSubView::Categories,
            reference_month,
            include_pending: false,
            category_cursor: 0,
            comparison_cursor: 0,
            period_modal: None,
            data: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalsSubView {
    #[default]
    Goals,
    Forecast,
}

impl GoalsSubView {
    pub const ALL: [GoalsSubView; 2] = [GoalsSubView::Goals, GoalsSubView::Forecast];

    pub fn title(&self) -> &'static str {
        match self {
            GoalsSubView::Goals => "1: Metas de Economia",
            GoalsSubView::Forecast => "2: Projeção de Fluxo de Caixa",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContributionFormField {
    #[default]
    Amount,
    Date,
    Note,
}

impl ContributionFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::Amount => Self::Date,
            Self::Date => Self::Note,
            Self::Note => Self::Amount,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Amount => Self::Note,
            Self::Date => Self::Amount,
            Self::Note => Self::Date,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionModalState {
    pub goal_id: GoalId,
    pub goal_name: String,
    pub amount_input: String,
    pub date_input: String,
    pub note_input: String,
    pub focused_field: ContributionFormField,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoalsTabState {
    pub active_subview: GoalsSubView,
    pub goals: Vec<GoalProgress>,
    pub selected_goal_index: usize,
    pub forecast: Option<CashflowForecast>,
    pub forecast_months: u32,
    pub forecast_granularity: ForecastGranularity,
    pub include_goals: bool,
    pub forecast_cursor: usize,
    pub contribution_modal: Option<ContributionModalState>,
}

impl Default for GoalsTabState {
    fn default() -> Self {
        Self {
            active_subview: GoalsSubView::Goals,
            goals: Vec::new(),
            selected_goal_index: 0,
            forecast: None,
            forecast_months: 6,
            forecast_granularity: ForecastGranularity::Month,
            include_goals: false,
            forecast_cursor: 0,
            contribution_modal: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub active_tab: Tab,
    pub running: bool,
    pub status_message: String,
    pub is_loading: bool,
    pub tick_count: u64,
    pub last_tick: DateTime<Local>,
    pub data_summary: Option<String>,
    pub dashboard_data: Option<DashboardData>,
    pub transactions_state: TransactionsTabState,
    pub reports_state: ReportsTabState,
    pub goals_state: GoalsTabState,
    pub error_message: Option<String>,
    pub is_help_open: bool,
    pub help_scroll: usize,
    pub theme: Theme,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    pub fn new() -> Self {
        Self::new_with_theme(ThemeMode::Dark)
    }

    pub fn new_with_theme(theme_mode: ThemeMode) -> Self {
        Self {
            active_tab: Tab::Dashboard,
            running: true,
            status_message: "Pronto".to_string(),
            is_loading: false,
            tick_count: 0,
            last_tick: Local::now(),
            data_summary: None,
            dashboard_data: None,
            transactions_state: TransactionsTabState::default(),
            reports_state: ReportsTabState::default(),
            goals_state: GoalsTabState::default(),
            error_message: None,
            is_help_open: false,
            help_scroll: 0,
            theme: Theme::from_mode(theme_mode),
        }
    }
}
