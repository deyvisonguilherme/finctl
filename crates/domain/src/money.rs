use crate::errors::DomainError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Money(Decimal);

impl Money {
    pub const ZERO: Money = Money(Decimal::ZERO);

    /// Cria uma nova quantia positiva (> 0) com no máximo 2 casas decimais.
    pub fn new(value: Decimal) -> Result<Self, DomainError> {
        if value <= Decimal::ZERO {
            return Err(DomainError::InvalidMoney(
                "O valor monetário deve ser maior que zero.".to_string(),
            ));
        }
        if value.scale() > 2 {
            return Err(DomainError::InvalidMoney(
                "O valor monetário não pode ter mais de 2 casas decimais.".to_string(),
            ));
        }
        Ok(Self(value.trunc_with_scale(2)))
    }

    /// Cria uma quantia não-negativa (>= 0), útil para saldo inicial.
    pub fn from_decimal_non_negative(value: Decimal) -> Result<Self, DomainError> {
        if value < Decimal::ZERO {
            return Err(DomainError::InvalidMoney(
                "O valor monetário não pode ser negativo.".to_string(),
            ));
        }
        if value.scale() > 2 {
            return Err(DomainError::InvalidMoney(
                "O valor monetário não pode ter mais de 2 casas decimais.".to_string(),
            ));
        }
        Ok(Self(value.trunc_with_scale(2)))
    }

    pub fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Faz o parsing de strings como "1500", "1500,50", "1.500,50", "1500.50", "R$ 1.234,56"
    pub fn parse(input: &str) -> Result<Self, DomainError> {
        let clean = input
            .trim()
            .trim_start_matches("R$")
            .trim_start_matches("r$")
            .trim();

        // Se tem ponto e vírgula (ex: 1.234,56), remove o ponto de milhar e substitui vírgula por ponto
        let normalized = if clean.contains('.') && clean.contains(',') {
            clean.replace('.', "").replace(',', ".")
        } else if clean.contains(',') {
            clean.replace(',', ".")
        } else {
            clean.to_string()
        };

        let decimal = Decimal::from_str(&normalized).map_err(|e| {
            DomainError::InvalidMoney(format!("Formato numérico inválido '{input}': {e}"))
        })?;

        Self::new(decimal)
    }

    /// Formata no padrão brasileiro: `R$ 1.234,56` ou `-R$ 1.234,56`
    pub fn format_pt_br(&self) -> String {
        format_decimal_pt_br(self.0)
    }
}

pub fn format_decimal_pt_br(decimal: Decimal) -> String {
    let is_negative = decimal < Decimal::ZERO;
    let abs_dec = decimal.abs();
    let rounded = abs_dec.round_dp(2);
    let s = format!("{:.2}", rounded);
    let parts: Vec<&str> = s.split('.').collect();
    let int_part = parts[0];
    let frac_part = parts.get(1).unwrap_or(&"00");

    let mut formatted_int = String::new();
    let chars: Vec<char> = int_part.chars().rev().collect();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            formatted_int.push('.');
        }
        formatted_int.push(*ch);
    }
    let formatted_int: String = formatted_int.chars().rev().collect();

    if is_negative {
        format!("-R$ {formatted_int},{frac_part}")
    } else {
        format!("R$ {formatted_int},{frac_part}")
    }
}

impl Add for Money {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Money(self.0 + rhs.0)
    }
}

impl Sub for Money {
    type Output = Result<Self, DomainError>;

    fn sub(self, rhs: Self) -> Self::Output {
        let diff = self.0 - rhs.0;
        if diff < Decimal::ZERO {
            Err(DomainError::InvalidMoney(
                "Subtração resultou em valor negativo.".to_string(),
            ))
        } else {
            Ok(Money(diff))
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_pt_br())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_valid_money_creation() {
        let m = Money::new(dec!(100.50)).unwrap();
        assert_eq!(m.as_decimal(), dec!(100.50));
    }

    #[test]
    fn test_rejects_negative_and_zero_for_positive_money() {
        assert!(Money::new(dec!(-10.00)).is_err());
        assert!(Money::new(dec!(0.00)).is_err());
    }

    #[test]
    fn test_allows_zero_for_non_negative() {
        let m = Money::from_decimal_non_negative(dec!(0.00)).unwrap();
        assert_eq!(m.as_decimal(), dec!(0.00));
    }

    #[test]
    fn test_rejects_more_than_two_decimal_places() {
        assert!(Money::new(dec!(10.555)).is_err());
        assert!(Money::from_decimal_non_negative(dec!(0.123)).is_err());
    }

    #[test]
    fn test_parse_formats() {
        assert_eq!(Money::parse("1500,50").unwrap().as_decimal(), dec!(1500.50));
        assert_eq!(
            Money::parse("1.500,50").unwrap().as_decimal(),
            dec!(1500.50)
        );
        assert_eq!(Money::parse("1500.50").unwrap().as_decimal(), dec!(1500.50));
        assert_eq!(
            Money::parse("R$ 1.234,56").unwrap().as_decimal(),
            dec!(1234.56)
        );
        assert_eq!(Money::parse("10").unwrap().as_decimal(), dec!(10));
    }

    #[test]
    fn test_format_pt_br() {
        let m = Money::new(dec!(1234.56)).unwrap();
        assert_eq!(m.format_pt_br(), "R$ 1.234,56");

        let m2 = Money::new(dec!(1000000.00)).unwrap();
        assert_eq!(m2.format_pt_br(), "R$ 1.000.000,00");

        let m3 = Money::new(dec!(5.50)).unwrap();
        assert_eq!(m3.format_pt_br(), "R$ 5,50");
    }

    #[test]
    fn test_add() {
        let a = Money::new(dec!(10.50)).unwrap();
        let b = Money::new(dec!(20.25)).unwrap();
        assert_eq!((a + b).as_decimal(), dec!(30.75));
    }
}
