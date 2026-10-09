use crate::errors::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new(id: Uuid) -> Self {
                Self(id)
            }

            pub fn generate() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl FromStr for $name {
            type Err = DomainError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|e| DomainError::Validation(format!("UUID inválido '{s}': {e}")))
            }
        }
    };
}

define_id!(UserId);
define_id!(AccountId);
define_id!(CategoryId);
define_id!(TransactionId);
define_id!(BudgetId);
define_id!(RecurringRuleId);
define_id!(CardInvoiceId);
define_id!(TagId);
define_id!(AttachmentId);
define_id!(GoalId);
define_id!(ContributionId);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    #[default]
    Paid,
    Pending,
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Paid => "paid",
            TransactionStatus::Pending => "pending",
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            TransactionStatus::Paid => "Realizado",
            TransactionStatus::Pending => "Previsto",
        }
    }
}

impl FromStr for TransactionStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "paid" | "realizado" | "pago" => Ok(TransactionStatus::Paid),
            "pending" | "previsto" | "pendente" => Ok(TransactionStatus::Pending),
            _ => Err(DomainError::Validation(format!(
                "Status de transação inválido '{s}'. Use 'paid' ou 'pending'."
            ))),
        }
    }
}

impl fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum InvoiceStatus {
    #[default]
    Open,
    Closed,
    Paid,
}

impl InvoiceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Paid => "paid",
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            Self::Open => "Aberta",
            Self::Closed => "Fechada",
            Self::Paid => "Paga",
        }
    }
}

impl FromStr for InvoiceStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "open" | "aberta" | "aberto" => Ok(Self::Open),
            "closed" | "fechada" | "fechado" => Ok(Self::Closed),
            "paid" | "paga" | "pago" => Ok(Self::Paid),
            _ => Err(DomainError::Validation(format!(
                "Status de fatura inválido '{s}'. Use 'open', 'closed' ou 'paid'."
            ))),
        }
    }
}

impl fmt::Display for InvoiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionKind {
    Income,
    Expense,
}

impl TransactionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionKind::Income => "income",
            TransactionKind::Expense => "expense",
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            TransactionKind::Income => "Receita",
            TransactionKind::Expense => "Despesa",
        }
    }
}

impl FromStr for TransactionKind {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "income" | "receita" | "rec" => Ok(TransactionKind::Income),
            "expense" | "despesa" | "desp" => Ok(TransactionKind::Expense),
            _ => Err(DomainError::Validation(format!(
                "Tipo de transação inválido '{s}'. Use 'income' ou 'expense'."
            ))),
        }
    }
}

impl fmt::Display for TransactionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Checking,
    Savings,
    Wallet,
    Investment,
    CreditCard,
}

impl AccountKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountKind::Checking => "checking",
            AccountKind::Savings => "savings",
            AccountKind::Wallet => "wallet",
            AccountKind::Investment => "investment",
            AccountKind::CreditCard => "credit_card",
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            AccountKind::Checking => "Conta Corrente",
            AccountKind::Savings => "Poupança",
            AccountKind::Wallet => "Carteira",
            AccountKind::Investment => "Investimento",
            AccountKind::CreditCard => "Cartão de Crédito",
        }
    }
}

impl FromStr for AccountKind {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "checking" | "corrente" | "conta_corrente" => Ok(AccountKind::Checking),
            "savings" | "poupanca" | "poupança" => Ok(AccountKind::Savings),
            "wallet" | "carteira" => Ok(AccountKind::Wallet),
            "investment" | "investimento" => Ok(AccountKind::Investment),
            "credit_card" | "cartao_de_credito" | "cartão_de_crédito" | "cartao" | "credito" => {
                Ok(AccountKind::CreditCard)
            }
            _ => Err(DomainError::Validation(format!(
                "Tipo de conta inválido '{s}'. Opções válidas: checking, savings, wallet, investment, credit_card."
            ))),
        }
    }
}

impl fmt::Display for AccountKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum RecurringFrequency {
    Weekly,
    #[default]
    Monthly,
    Yearly,
}

impl RecurringFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    pub fn display_pt_br(&self) -> &'static str {
        match self {
            Self::Weekly => "Semanal",
            Self::Monthly => "Mensal",
            Self::Yearly => "Anual",
        }
    }
}

impl FromStr for RecurringFrequency {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "weekly" | "semanal" | "semana" => Ok(Self::Weekly),
            "monthly" | "mensal" | "mes" | "mês" => Ok(Self::Monthly),
            "yearly" | "anual" | "ano" => Ok(Self::Yearly),
            _ => Err(DomainError::Validation(format!(
                "Frequência recorrente inválida '{s}'. Use: weekly, monthly ou yearly."
            ))),
        }
    }
}

impl fmt::Display for RecurringFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_kind_parsing() {
        assert_eq!(
            "income".parse::<TransactionKind>().unwrap(),
            TransactionKind::Income
        );
        assert_eq!(
            "receita".parse::<TransactionKind>().unwrap(),
            TransactionKind::Income
        );
        assert_eq!(
            "expense".parse::<TransactionKind>().unwrap(),
            TransactionKind::Expense
        );
        assert_eq!(
            "despesa".parse::<TransactionKind>().unwrap(),
            TransactionKind::Expense
        );
        assert!("invalid".parse::<TransactionKind>().is_err());
    }

    #[test]
    fn test_account_kind_parsing() {
        assert_eq!(
            "checking".parse::<AccountKind>().unwrap(),
            AccountKind::Checking
        );
        assert_eq!(
            "corrente".parse::<AccountKind>().unwrap(),
            AccountKind::Checking
        );
        assert_eq!(
            "savings".parse::<AccountKind>().unwrap(),
            AccountKind::Savings
        );
        assert_eq!(
            "wallet".parse::<AccountKind>().unwrap(),
            AccountKind::Wallet
        );
        assert_eq!(
            "investment".parse::<AccountKind>().unwrap(),
            AccountKind::Investment
        );
        assert_eq!(
            "credit_card".parse::<AccountKind>().unwrap(),
            AccountKind::CreditCard
        );
        assert_eq!(
            "cartao".parse::<AccountKind>().unwrap(),
            AccountKind::CreditCard
        );
        assert!("outra".parse::<AccountKind>().is_err());
    }

    #[test]
    fn test_transaction_status_parsing() {
        assert_eq!(
            "paid".parse::<TransactionStatus>().unwrap(),
            TransactionStatus::Paid
        );
        assert_eq!(
            "pending".parse::<TransactionStatus>().unwrap(),
            TransactionStatus::Pending
        );
        assert_eq!(
            "previsto".parse::<TransactionStatus>().unwrap(),
            TransactionStatus::Pending
        );
    }

    #[test]
    fn test_typed_ids() {
        let u = Uuid::new_v4();
        let acc_id = AccountId::new(u);
        assert_eq!(acc_id.as_uuid(), u);
        assert_eq!(acc_id.to_string(), u.to_string());
    }
}
