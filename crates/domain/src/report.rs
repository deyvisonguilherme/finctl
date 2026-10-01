use crate::{CategoryId, Money, TransactionKind};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonthlyReportItem {
    pub month: String,
    pub total_income: Money,
    pub total_expense: Money,
    pub net_balance: Decimal,
    pub savings_rate: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryReportItem {
    pub category_id: CategoryId,
    pub category_name: String,
    pub parent_id: Option<CategoryId>,
    pub parent_name: Option<String>,
    pub kind: TransactionKind,
    pub total_amount: Money,
    pub transaction_count: i64,
    pub percentage: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryReportSummary {
    pub kind: Option<TransactionKind>,
    pub total_amount: Money,
    pub items: Vec<CategoryReportItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryComparisonRow {
    pub category_name: String,
    pub kind: TransactionKind,
    pub monthly_amounts: Vec<(String, Money)>,
    pub absolute_diff: Decimal,
    pub percent_diff: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryComparisonReport {
    pub months: Vec<String>,
    pub rows: Vec<CategoryComparisonRow>,
}
