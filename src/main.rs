use std::{
    collections::{HashMap, VecDeque},
    sync::Arc, env,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::RwLock;

#[derive(Default, Clone)]
struct Account {
    balance: i64,
    limit: i64,
    transactions: RingBuffer<Transaction>,
}

#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize)]
struct RingBuffer<T>(VecDeque<T>);

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::with_capacity(10)
    }
}

// NAO FACAM ISSO
impl<T> RingBuffer<T> {
    fn with_capacity(capacity: usize) -> Self {
        Self(VecDeque::with_capacity(capacity))
    }

    fn push(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_back();
            self.0.push_front(item);
        } else {
            self.0.push_front(item);
        }
    }
}

impl Account {
    pub fn with_limit(limit: i64) -> Self {
        Account {
            limit,
            ..Default::default()
        }
    }

    pub fn transact(&mut self, transaction: Transaction) -> Result<(), &'static str> {
        match transaction.kind {
            TransactionType::Credit => {
                self.balance += transaction.value;
                self.transactions.push(transaction);
                Ok(())
            }
            TransactionType::Debit => {
                if self.balance + self.limit >= transaction.value {
                    self.balance -= transaction.value;
                    self.transactions.push(transaction);
                    Ok(())
                } else {
                    Err("Não tem limite o suficiente")
                }
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
enum TransactionType {
    #[serde(rename = "c")]
    Credit,
    #[serde(rename = "d")]
    Debit,
}

#[derive(Clone, Serialize, Deserialize)]
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

    let accounts = HashMap::<u8, RwLock<Account>>::from_iter([
        (1, RwLock::new(Account::with_limit(100_000))),
        (2, RwLock::new(Account::with_limit(80_000))),
        (3, RwLock::new(Account::with_limit(1_000_000))),
        (4, RwLock::new(Account::with_limit(10_000_000))),
        (5, RwLock::new(Account::with_limit(500_000))),
    ]);

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
