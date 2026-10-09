use crate::theme::ThemeMode;
use app::{DashboardData, PaginatedTransactions, ReportsScreenData};
use crossterm::event::KeyEvent;
use domain::{CashflowForecast, GoalProgress};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Quit,
    Key(KeyEvent),
    NextTab,
    PreviousTab,
    SelectTab(usize),
    Tick,
    Resize(u16, u16),
    StatusMessage(String),
    SetLoading(bool),
    DataLoaded(String),
    DashboardLoaded(Box<DashboardData>),
    TransactionsLoaded(PaginatedTransactions),
    TransactionActionSuccess(String),
    ReportDataLoaded(Box<ReportsScreenData>),
    GoalsDataLoaded(Vec<GoalProgress>),
    ForecastDataLoaded(Box<CashflowForecast>),
    GoalContributionSuccess(String),
    ErrorOccurred(String),
    ToggleHelp,
    CloseHelp,
    ToggleTheme,
    SetTheme(ThemeMode),
}
