use std::time::Duration;

use eggfetch_core::{
    DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer,
    PhysicalConnectionPolicy, TransportIoTimeout,
};
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct LocalDialer {
    address: std::net::SocketAddr,
}

impl Dialer for LocalDialer {
    fn dial(&self, _target: DialTarget) -> DialFuture<'_> {
        let address = self.address;
        Box::pin(async move {
            let stream = TcpStream::connect(address).await.map_err(|error| {
                DialError::with_source(DialErrorKind::Connection, "fixture connect failed", error)
            })?;
            Ok(Box::new(stream) as DialStream)
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = Vec::new();
        loop {
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).await?;
            request.push(byte[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<_, std::io::Error>(())
    });

    let client = eggfetch_core::Client::builder()
        .dialer(LocalDialer { address })
        .retry_canceled_requests(false)
        .physical_connection_policy(PhysicalConnectionPolicy {
            max_live: Some(1),
            admission_timeout: Some(Duration::from_secs(1)),
        })
        .transport_io_timeout(TransportIoTimeout {
            read: Some(Duration::from_secs(1)),
            write: Some(Duration::from_secs(1)),
        })
        .build();

    let mut response = client
        .get(format!("http://fixture.invalid:{}/", address.port()).as_str())?
        .send()
        .await?;
    let mut body = response.bytes_stream()?;
    let mut collected = Vec::new();
    while let Some(chunk) = body.next().await {
        collected.extend_from_slice(&chunk?);
    }
    assert_eq!(collected, b"ok");
    server.await??;
    Ok(())
}
