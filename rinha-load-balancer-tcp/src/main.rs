use tokio::{
    io,
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9999").await?;
    let addrs = ["api1:3000", "api2:3000"];
    let mut counter = 0;

    while let Ok((mut downstream, _)) = listener.accept().await {
        counter += 1;
        let addr = addrs[counter % addrs.len()];
        tokio::spawn(async move {
            let mut upstream = TcpStream::connect(addr).await.unwrap();
            io::copy_bidirectional(&mut downstream, &mut upstream)
                .await
                .unwrap();
        });
    }

    Ok(())
}
