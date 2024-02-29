use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use axum::{
    body::Body,
    extract::{Request, State},
    handler::Handler,
    http::{
        uri::{Authority, Scheme},
        StatusCode, Uri,
    },
    response::IntoResponse,
};
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    addrs: Vec<&'static str>,
    req_counter: Arc<AtomicUsize>,
    http_client: Client<HttpConnector, Body>,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:9999").await.unwrap();

    let addrs = ["0.0.0.0:9998", "0.0.0.0:9997"];

    let client = Client::builder(TokioExecutor::new()).build_http::<Body>();

    let app_state = AppState {
        addrs: addrs.to_vec(),
        req_counter: Arc::new(AtomicUsize::new(0)),
        http_client: client,
    };

    let app = proxy.with_state(app_state);

    axum::serve(listener, app).await.unwrap();
}

async fn proxy(
    State(AppState {
        addrs,
        req_counter,
        http_client,
    }): State<AppState>,
    mut req: Request,
) -> impl IntoResponse {
    let count = req_counter.fetch_add(1, Ordering::Relaxed);

    *req.uri_mut() = {
        let uri = req.uri();
        let mut parts = uri.clone().into_parts();
        parts.authority = Authority::from_str(addrs[count % addrs.len()]).ok();
        parts.scheme = Some(Scheme::HTTP);
        Uri::from_parts(parts).unwrap()
    };

    match http_client.request(req).await {
        Ok(res) => Ok(res),
        Err(_) => Err(StatusCode::BAD_GATEWAY),
    }
}
