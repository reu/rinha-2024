use std::{collections::HashMap, env, error::Error, path::Path as FilePath, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use espora_db::{Db, Error as DbError};
use ring_buffer::RingBuffer;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::RwLock;

mod ring_buffer;

struct Account {
    balance: i64,
    limit: i64,
    transactions: RingBuffer<Transaction, 10>,
    db: Db<(i64, Transaction), 128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct Description(String);

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

impl Account {
    pub fn with_db(path: impl AsRef<FilePath>, limit: i64) -> Result<Self, Box<dyn Error>> {
        let mut db = Db::<(i64, Transaction), 128>::from_path(path)?;

        let transactions = db
            .rows_reverse()
            .take(10)
            .collect::<Result<Vec<_>, DbError>>()?;

        let balance = transactions
            .first()
            .map(|(balance, _)| *balance)
            .unwrap_or_default();

        Ok(Account {
            limit,
            balance,
            transactions: transactions
                .into_iter()
                .map(|(_, transaction)| transaction)
                .collect(),
            db,
        })
    }

    pub fn transact(&mut self, transaction: Transaction) -> Result<(), &'static str> {
        let balance = match transaction.kind {
            TransactionType::Credit => self.balance + transaction.value,
            TransactionType::Debit => {
                if self.balance + self.limit >= transaction.value {
                    self.balance - transaction.value
                } else {
                    return Err("Não tem limite o suficiente");
                }
            }
        };
        self.db
            .insert((balance, transaction.clone()))
            .map_err(|_| "Erro ao persistir")?;
        self.balance = balance;
        self.transactions.push_front(transaction);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TransactionType {
    #[serde(rename = "c")]
    Credit,
    #[serde(rename = "d")]
    Debit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    #[serde(rename = "valor")]
    value: i64,
    #[serde(rename = "tipo")]
    kind: TransactionType,
    #[serde(rename = "descricao")]
    description: Description,
    #[serde(
        rename = "realizada_em",
        with = "time::serde::rfc3339",
        default = "OffsetDateTime::now_utc"
    )]
    created_at: OffsetDateTime,
}

type AppState = Arc<HashMap<u8, RwLock<Account>>>;

#[tokio::main]
async fn main() {
    let port = env::var("PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(9999);

    #[rustfmt::skip]
    let accounts = HashMap::from_iter([
        (1, RwLock::new(Account::with_db("account-1.espora", 100_000).unwrap())),
        (2, RwLock::new(Account::with_db("account-2.espora", 80_000).unwrap())),
        (3, RwLock::new(Account::with_db("account-3.espora", 1_000_000).unwrap())),
        (4, RwLock::new(Account::with_db("account-4.espora", 10_000_000).unwrap())),
        (5, RwLock::new(Account::with_db("account-5.espora", 500_000).unwrap())),
    ]);

    println!("Server ready {port}");

    let app = Router::new()
        .route("/clientes/:id/transacoes", post(create_transaction))
        .route("/clientes/:id/extrato", get(view_account))
        .with_state(Arc::new(accounts));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn create_transaction(
    Path(account_id): Path<u8>,
    State(accounts): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> impl IntoResponse {
    match accounts.get(&account_id) {
        Some(account) => {
            let mut account = account.write().await;
            match account.transact(transaction) {
                Ok(()) => Ok(Json(json!({
                    "limite": account.limit,
                    "saldo": account.balance,
                }))),
                Err(_) => Err(StatusCode::UNPROCESSABLE_ENTITY),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn view_account(
    Path(account_id): Path<u8>,
    State(accounts): State<AppState>,
) -> impl IntoResponse {
    match accounts.get(&account_id) {
        Some(account) => {
            let account = account.read().await;
            Ok(Json(json!({
                "saldo": {
                    "total": account.balance,
                    "data_extrato": OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
                    "limite": account.limit,
                },
                "ultimas_transacoes": account.transactions,
            })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}
