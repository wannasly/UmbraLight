use std::time::Duration;
use tokio::net::{lookup_host, TcpStream};
use tokio::time::{timeout, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const ATTEMPTS: u32 = 2;

/// Best (minimum) TCP connect time over two attempts; None when host does not
/// resolve or no attempt connects within 3s.
pub async fn tcp_ping(host: &str, port: u16) -> Option<u32> {
    let addr = lookup_host((host, port)).await.ok()?.next()?;
    let mut best: Option<u32> = None;
    for _ in 0..ATTEMPTS {
        let started = Instant::now();
        if let Ok(Ok(_stream)) = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
            let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            best = Some(best.map_or(ms, |b| b.min(ms)));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ping_local_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });
        let res = tcp_ping("127.0.0.1", port).await;
        assert!(res.is_some());
    }
}
