use app::{CreateTransactionInput, EditTransactionInput, ListTransactionsInput};
use chrono::NaiveDate;
use domain::TransactionId;

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
    Custom(String),
}
