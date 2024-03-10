use std::env;

use tokio::{
    io,
    net::{TcpListener, UnixStream},
};

#[tokio::main]
async fn main() -> io::Result<()> {
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
            String::from("./rinha-app1.socket"),
            String::from("./rinha-app2.socket"),
        ])
        .into_iter()
        .map(|addr| Box::leak(addr.into_boxed_str()) as &'static str)
        .collect::<Vec<_>>();

    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    let mut counter = 0;

    println!("TCP lb ({}) ready 9999", env!("CARGO_PKG_VERSION"));
    while let Ok((mut downstream, _)) = listener.accept().await {
        downstream.set_nodelay(true)?;
        counter += 1;
        let addr = addrs[counter % addrs.len()];
        tokio::spawn(async move {
            let mut upstream = UnixStream::connect(addr).await.unwrap();
            io::copy_bidirectional(&mut downstream, &mut upstream)
                .await
                .unwrap();
        });
    }

    Ok(())
}
