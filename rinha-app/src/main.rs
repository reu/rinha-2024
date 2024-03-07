use std::env;

use tokio::{
    io,
    net::{TcpListener, TcpStream},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> io::Result<()> {
    let port = env::var("PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(9998);

    let db = env::var("DB").unwrap_or(String::from("0.0.0.0:9999"));
    let db: &'static str = Box::leak(db.into_boxed_str());

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;

    println!("App ({VERSION}) ready {port}");

    while let Ok((mut downstream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let mut upstream = TcpStream::connect(db).await.unwrap();
            io::copy_bidirectional(&mut downstream, &mut upstream)
                .await
                .unwrap();
        });
    }

    Ok(())
}
