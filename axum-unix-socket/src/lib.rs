use axum::{extract::Request, response::Response};
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server,
};
use std::{convert::Infallible, io, path::Path};
use tokio::{fs, net::UnixListener};
use tower::Service;

pub async fn serve<S>(path: impl AsRef<Path>, app: S) -> io::Result<()>
where
    S: Service<Request<Incoming>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    let path = path.as_ref();

    fs::remove_file(&path).await.ok();

    let listener = UnixListener::bind(path)?;

    while let Ok((socket, _addr)) = listener.accept().await {
        let service = app.clone();

        tokio::spawn(async move {
            let socket = TokioIo::new(socket);

            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                service.clone().call(request)
            });

            if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(socket, hyper_service)
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }

    Ok(())
}
