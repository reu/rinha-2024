use std::{
    collections::{HashMap, VecDeque},
    env,
    error::Error,
    path::Path as FilePath,
    sync::Arc,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use espora_db::Db;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::RwLock;

struct Account {
    balance: i64,
    limit: i64,
    transactions: RingBuffer<Transaction>,
    db: Db<(i64, Transaction), 1024>,
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

impl<T> RingBuffer<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(VecDeque::with_capacity(capacity))
    }

    pub fn push(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_back();
            self.0.push_front(item);
        } else {
            self.0.push_front(item);
        }
    }
}

impl<A> FromIterator<A> for RingBuffer<A> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let mut ring_buffer = Self::with_capacity(10);
        for item in iter.into_iter() {
            ring_buffer.push(item)
        }
        ring_buffer
    }
}

impl Account {
    pub fn with_db(path: impl AsRef<FilePath>, limit: i64) -> Result<Self, Box<dyn Error>> {
        let mut db = Db::<(i64, Transaction), 1024>::from_path(path)?;

        let mut transactions = db.rows().collect::<Vec<_>>();

        let balance = transactions
            .last()
            .map(|(balance, _)| *balance)
            .unwrap_or_default();

        transactions.reverse();

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
        self.transactions.push(transaction);
        Ok(())
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

    #[rustfmt::skip]
    let accounts = HashMap::<u8, RwLock<Account>>::from_iter([
        (1, RwLock::new(Account::with_db("account-1.espora", 100_000).unwrap())),
        (2, RwLock::new(Account::with_db("account-2.espora", 80_000).unwrap())),
        (3, RwLock::new(Account::with_db("account-3.espora", 1_000_000).unwrap())),
        (4, RwLock::new(Account::with_db("account-4.espora", 10_000_000).unwrap())),
        (5, RwLock::new(Account::with_db("account-5.espora", 500_000).unwrap())),
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
