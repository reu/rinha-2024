use std::env;

use tokio::{
    fs, io,
    net::{UnixListener, UnixStream},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let unix_socket = env::var("UNIX_SOCKET")
        .ok()
        .unwrap_or(String::from("./rinha-app.socket"));

    let db = env::var("DB").unwrap_or(String::from("./rinha-espora-server.socket"));
    let db: &'static str = Box::leak(db.into_boxed_str());

    fs::remove_file(&unix_socket).await.ok();
    let listener = UnixListener::bind(&unix_socket)?;

    println!("App ({}) ready {unix_socket}", env!("CARGO_PKG_VERSION"));

    while let Ok((mut downstream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let mut upstream = UnixStream::connect(db).await.unwrap();
            io::copy_bidirectional(&mut downstream, &mut upstream)
                .await
                .unwrap();
        });
    }

    Ok(())
}
