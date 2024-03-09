use std::{convert::TryFrom, fmt::Display};

use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Description(String);

impl TryFrom<String> for Description {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > 10 {
            Err("Descricao inválida")
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    #[serde(rename = "c")]
    Credit,
    #[serde(rename = "d")]
    Debit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "valor")]
    pub value: i64,
    #[serde(rename = "tipo")]
    pub kind: TransactionType,
    #[serde(rename = "descricao")]
    pub description: Description,
    #[serde(rename = "realizada_em", default)]
    pub created_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTime(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);

impl Default for DateTime {
    fn default() -> Self {
        Self(OffsetDateTime::now_utc())
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format(&Rfc3339).unwrap())
    }
}

impl DateTime {
    pub fn now() -> DateTime {
        Default::default()
    }
}
