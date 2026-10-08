use app::{DashboardData, PaginatedTransactions};
use crossterm::event::KeyEvent;

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
    ErrorOccurred(String),
}
