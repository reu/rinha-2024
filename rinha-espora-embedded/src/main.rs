use std::{
    collections::HashMap,
    env,
    path::{Path as FilePath, PathBuf},
    sync::Arc,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use espora_db::{tokio::Db, Error as DbError};
use futures::{StreamExt, TryStreamExt};
use rinha::{DateTime, Transaction, TransactionType};
use serde_json::json;
use tokio::sync::Mutex;

type Balance = i64;

struct Account {
    limit: i64,
    db: Db<(Balance, Transaction), 128>,
}

impl Account {
    pub async fn with_db(path: impl AsRef<FilePath>, limit: i64) -> Result<Self, DbError> {
        let db = Db::<(i64, Transaction), 128>::builder()
            .sync_write(option_env!("ESPORA_SYNC_WRITE") == Some("1"))
            .build_tokio(&path)
            .await?;

        Ok(Account { limit, db })
    }

    pub async fn transact(&mut self, transaction: Transaction) -> Result<Balance, &'static str> {
        let lock = self
            .db
            .lock_writes()
            .await
            .map_err(|_| "Falha ao conseguir o lock do db")?;

        let current_balance = self
            .db
            .rows_reverse()
            .take(1)
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| "Falha ao ler do db")?
            .first()
            .map(|(balance, _)| *balance)
            .unwrap_or(0);

        let balance = match transaction.kind {
            TransactionType::Credit => current_balance + transaction.value,
            TransactionType::Debit => {
                if current_balance + self.limit >= transaction.value {
                    current_balance - transaction.value
                } else {
                    return Err("Não tem limite o suficiente");
                }
            }
        };

        self.db
            .insert((balance, transaction.clone()))
            .await
            .map_err(|_| "Erro ao persistir no db")?;

        drop(lock);

        Ok(balance)
    }

    pub async fn last_transactions(
        &mut self,
        limit: usize,
    ) -> Result<Vec<(i64, Transaction)>, DbError> {
        self.db
            .rows_reverse()
            .take(limit)
            .try_collect::<Vec<_>>()
            .await
    }
}

type AppState = Arc<HashMap<u8, Mutex<Account>>>;

#[tokio::main]
async fn main() {
    let unix_socket = env::var("UNIX_SOCKET")
        .ok()
        .unwrap_or(String::from("./rinha-espora-app.socket"));

    let db = env::var("DB")
        .ok()
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("./"));

    #[rustfmt::skip]
    let accounts = HashMap::from_iter([
        (1, Mutex::new(Account::with_db(db.join("account-1.espora"), 100_000).await.unwrap())),
        (2, Mutex::new(Account::with_db(db.join("account-2.espora"), 80_000).await.unwrap())),
        (3, Mutex::new(Account::with_db(db.join("account-3.espora"), 1_000_000).await.unwrap())),
        (4, Mutex::new(Account::with_db(db.join("account-4.espora"), 10_000_000).await.unwrap())),
        (5, Mutex::new(Account::with_db(db.join("account-5.espora"), 500_000).await.unwrap())),
    ]);

    let app = Router::new()
        .route("/clientes/:id/transacoes", post(create_transaction))
        .route("/clientes/:id/extrato", get(view_account))
        .with_state(Arc::new(accounts));

    println!("App ({}) ready {unix_socket}", env!("CARGO_PKG_VERSION"));

    axum_unix_socket::serve(unix_socket, app).await.unwrap();
}

async fn create_transaction(
    Path(account_id): Path<u8>,
    State(accounts): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> impl IntoResponse {
    match accounts.get(&account_id) {
        Some(account) => {
            let mut account = account.lock().await;
            match account.transact(transaction).await {
                Ok(balance) => Ok(Json(json!({
                    "limite": account.limit,
                    "saldo": balance,
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
            let mut account = account.lock().await;

            let transactions = account.last_transactions(10).await.unwrap_or_default();

            let balance = transactions
                .first()
                .map(|(balance, _)| *balance)
                .unwrap_or_default();

            let transactions = transactions
                .into_iter()
                .map(|(_, txn)| txn)
                .collect::<Vec<_>>();

            Ok(Json(json!({
                "saldo": {
                    "total": balance,
                    "data_extrato": DateTime::now(),
                    "limite": account.limit,
                },
                "ultimas_transacoes": transactions,
            })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}
