use app::{
    AddContributionInput, CreateTransactionInput, EditTransactionInput, ListTransactionsInput,
};
use chrono::NaiveDate;
use domain::{ForecastGranularity, TransactionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    FetchInitialData,
    RefreshData,
    FetchTransactions(ListTransactionsInput),
    PayTransactions(Vec<TransactionId>, Option<NaiveDate>),
    DeleteTransactions(Vec<TransactionId>),
    CreateTransaction(CreateTransactionInput),
    EditTransaction(EditTransactionInput),
    FetchReportData {
        month: String,
        include_pending: bool,
    },
    FetchGoalsData,
    FetchForecastData {
        months: u32,
        granularity: ForecastGranularity,
        include_goals: bool,
    },
    AddGoalContribution(AddContributionInput),
    Custom(String),
}
