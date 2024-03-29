use std::{
    env,
    hash::{DefaultHasher, Hash, Hasher},
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
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
    load_balancer: Arc<dyn LoadBalancer + Send + Sync>,
    http_client: Client<HttpConnector, Body>,
}

struct RoundRobin {
    addrs: Vec<String>,
    req_counter: Arc<AtomicUsize>,
}

trait LoadBalancer {
    fn next_server(&self, req: &Request) -> String;
}

impl LoadBalancer for RoundRobin {
    fn next_server(&self, _req: &Request) -> String {
        let count = self.req_counter.fetch_add(1, Ordering::Relaxed);
        self.addrs[count % self.addrs.len()].clone()
    }
}

struct RinhaAccountBalancer {
    addrs: Vec<String>,
}

impl LoadBalancer for RinhaAccountBalancer {
    fn next_server(&self, req: &Request) -> String {
        let path = req.uri().path();
        let hash = {
            let mut hasher = DefaultHasher::new();
            path.hash(&mut hasher);
            hasher.finish() as usize
        };
        self.addrs[hash % self.addrs.len()].to_string()
    }
}

#[tokio::main]
async fn main() {
    let port = env::var("PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(9999);

    let addrs = env::var("UPSTREAMS")
        .ok()
        .map(|upstream| {
            upstream
                .split(',')
                .map(|addr| addr.trim().to_owned())
                .collect::<Vec<String>>()
        })
        .unwrap_or(vec![
            String::from("0.0.0.0:9997"),
            String::from("0.0.0.0:9998"),
        ]);

    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();

    let client = {
        let mut connector = HttpConnector::new();
        connector.set_keepalive(Some(Duration::from_secs(60)));
        connector.set_nodelay(true);
        Client::builder(TokioExecutor::new())
            .http2_only(true)
            .build::<_, Body>(connector)
    };

    #[allow(unused)]
    let round_robin = RoundRobin {
        addrs: addrs.clone(),
        req_counter: Arc::new(AtomicUsize::new(0)),
    };

    #[allow(unused)]
    let fixed_load_balancer = RinhaAccountBalancer {
        addrs: addrs.clone(),
    };

    let app_state = AppState {
        load_balancer: Arc::new(round_robin),
        http_client: client,
    };

    let app = proxy.with_state(app_state);

    println!("HTTP lb ({}) ready 9999", env!("CARGO_PKG_VERSION"));

    axum::serve(listener, app).await.unwrap();
}

async fn proxy(
    State(AppState {
        load_balancer,
        http_client,
    }): State<AppState>,
    mut req: Request,
) -> impl IntoResponse {
    let addr = load_balancer.next_server(&req);

    *req.uri_mut() = {
        let uri = req.uri();
        let mut parts = uri.clone().into_parts();
        parts.authority = Authority::from_str(addr.as_str()).ok();
        parts.scheme = Some(Scheme::HTTP);
        Uri::from_parts(parts).unwrap()
    };

    match http_client.request(req).await {
        Ok(res) => Ok(res),
        Err(_) => Err(StatusCode::BAD_GATEWAY),
    }
}
