#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    FetchInitialData,
    RefreshData,
    Custom(String),
}
