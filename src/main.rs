use std::{io::Write, net::SocketAddr, path::PathBuf, str::FromStr, time::Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_native_tls::*;

fn server(path: PathBuf) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let mut f = tokio::fs::File::open(path).await.unwrap();
            let size = f.metadata().await.unwrap().len();

            let socket = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();

            let tls = {
                TlsAcceptor::from(
                    native_tls::TlsAcceptor::builder(
                        native_tls::Identity::from_pkcs12(include_bytes!("identity.p12"), "mypass")
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                )
            };

            println!("Listening on {}", socket.local_addr().unwrap());

            let (tx, _) = socket.accept().await.unwrap();
            let mut tx = tls.accept(tx).await.unwrap();

            tx.write_u64_le(size).await.unwrap();

            let start = Instant::now();
            tokio::io::copy(&mut f, &mut tx).await.unwrap();
            let start = start.elapsed();
            println!(
                "File sent in {start:?} ({} bytes/s)",
                size as f64 / start.as_secs_f64()
            );
        });
}

fn client() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let ip_port = {
                print!("Enter peer IP address and port: ");
                std::io::stdout().lock().flush().unwrap();

                let mut addr = String::new();
                std::io::stdin().read_line(&mut addr).unwrap();

                SocketAddr::from_str(addr.trim()).unwrap()
            };

            let socket = tokio::net::TcpStream::connect(ip_port).await.unwrap();

            let mut rx = TlsConnector::from(
                native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .disable_built_in_roots(true)
                    .use_sni(false)
                    .build()
                    .unwrap(),
            )
            .connect("tcptransfer", socket)
            .await
            .unwrap();

            let size = rx.read_u64_le().await.unwrap();

            let mut f = tokio::fs::File::create(std::env::temp_dir().join("received.bin"))
                .await
                .unwrap();

            let start = Instant::now();
            tokio::io::copy(&mut rx.take(size), &mut f).await.unwrap();
            let start = start.elapsed();
            println!(
                "File received in {start:?} ({} bytes/s)",
                size as f64 / start.as_secs_f64()
            );
        });
}

fn main() {
    let path = {
        print!("Enter file path or press enter to receive: ");
        std::io::stdout().lock().flush().unwrap();

        let mut path = String::new();
        std::io::stdin().read_line(&mut path).unwrap();

        let path = path.trim();

        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    };

    if let Some(path) = path {
        server(path);
    } else {
        client();
    }
}
