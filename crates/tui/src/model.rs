use app::DashboardData;
use chrono::{DateTime, Local};

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
    pub error_message: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Dashboard,
            running: true,
            status_message: "Pronto".to_string(),
            is_loading: false,
            tick_count: 0,
            last_tick: Local::now(),
            data_summary: None,
            dashboard_data: None,
            error_message: None,
        }
    }
}
