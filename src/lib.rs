use std::convert::TryFrom;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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
    #[serde(
        rename = "realizada_em",
        with = "time::serde::rfc3339",
        default = "OffsetDateTime::now_utc"
    )]
    pub created_at: OffsetDateTime,
}
