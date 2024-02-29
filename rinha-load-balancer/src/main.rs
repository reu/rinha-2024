use std::{
    hash::{DefaultHasher, Hash, Hasher},
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
    load_balancer: Arc<dyn LoadBalancer + Send + Sync>,
    http_client: Client<HttpConnector, Body>,
}

struct RoundRobin {
    addrs: Vec<&'static str>,
    req_counter: Arc<AtomicUsize>,
}

trait LoadBalancer {
    fn next_server(&self, req: &Request) -> String;
}

impl LoadBalancer for RoundRobin {
    fn next_server(&self, _req: &Request) -> String {
        let count = self.req_counter.fetch_add(1, Ordering::Relaxed);
        self.addrs[count % self.addrs.len()].to_string()
    }
}

struct RinhaAccountBalancer {
    addrs: Vec<&'static str>,
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
    let listener = TcpListener::bind("0.0.0.0:9999").await.unwrap();

    let addrs = ["0.0.0.0:9998", "0.0.0.0:9997"];

    let client = Client::builder(TokioExecutor::new()).build_http::<Body>();

    #[allow(unused)]
    let round_robin = RoundRobin {
        addrs: addrs.to_vec(),
        req_counter: Arc::new(AtomicUsize::new(0)),
    };

    #[allow(unused)]
    let fixed_load_balancer = RinhaAccountBalancer {
        addrs: addrs.to_vec(),
    };

    let app_state = AppState {
        load_balancer: Arc::new(round_robin),
        http_client: client,
    };

    let app = proxy.with_state(app_state);

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
