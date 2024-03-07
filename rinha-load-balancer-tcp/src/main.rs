use tokio::{
    io,
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9999").await?;
    let addrs = ["0.0.0.0:9997", "0.0.0.0:9998"];
    let mut counter = 0;

    println!("TCP lb ready 9999");
    while let Ok((mut downstream, _)) = listener.accept().await {
        downstream.set_nodelay(true)?;
        counter += 1;
        let addr = addrs[counter % addrs.len()];
        tokio::spawn(async move {
            let mut upstream = TcpStream::connect(addr).await.unwrap();
            upstream.set_nodelay(true).unwrap();
            io::copy_bidirectional(&mut downstream, &mut upstream)
                .await
                .unwrap();
        });
    }

    Ok(())
}
