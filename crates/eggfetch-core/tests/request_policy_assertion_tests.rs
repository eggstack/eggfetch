#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::large_futures
)]
//! Tests for the always-available request-policy assertions.
//!
//! `without_retry()` and `redirect_policy()` are disable switches, so
//! they must be callable in every profile. A profile that compiles out the
//! retry engine or the redirect loop has no such capability to disable, so
//! the setters are semantic no-ops there; this suite pins both halves: the
//! call sites compile everywhere, and the values are honored exactly as
//! before when the corresponding feature is enabled.

#[cfg(feature = "high-level-url")]
mod high_level {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use eggfetch_core::{Client, RedirectPolicy};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    /// Loopback server that answers every request with a fixed status.
    struct CountingServer {
        port: u16,
        shutdown: tokio::sync::watch::Sender<bool>,
        request_count: Arc<AtomicUsize>,
        status: u16,
        location: Option<String>,
    }

    impl CountingServer {
        async fn start(status: u16, location: Option<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
            let request_count = Arc::new(AtomicUsize::new(0));
            let counter = request_count.clone();
            let task_location = location.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        result = listener.accept() => {
                            let Ok((mut stream, _)) = result else { break };
                            counter.fetch_add(1, Ordering::SeqCst);
                            let location = task_location.clone();
                            tokio::spawn(async move {
                                let mut reader = BufReader::new(&mut stream);
                                let mut request_line = String::new();
                                reader.read_line(&mut request_line).await.ok();
                                loop {
                                    let mut line = String::new();
                                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                                        break;
                                    }
                                    if line.trim().is_empty() {
                                        break;
                                    }
                                }
                                // The redirect target answers 200 so the
                                // redirect chain terminates after one hop.
                                let is_terminal = request_line
                                    .split_whitespace()
                                    .nth(1)
                                    .is_some_and(|path| path == "/final");
                                let (status, extra) = if is_terminal {
                                    (200u16, String::new())
                                } else {
                                    (
                                        status,
                                        location
                                            .as_ref()
                                            .map(|value| format!("Location: {value}\r\n"))
                                            .unwrap_or_default(),
                                    )
                                };
                                let reason = if status == 200 { "OK" } else { "Status" };
                                let response = format!(
                                    "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\n{extra}Connection: close\r\n\r\n"
                                );
                                stream.write_all(response.as_bytes()).await.ok();
                            });
                        }
                        _ = shutdown_rx.changed() => break,
                    }
                }
            });

            Self {
                port,
                shutdown: shutdown_tx,
                request_count,
                status,
                location,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://127.0.0.1:{}{path}", self.port)
        }
    }

    /// The policy setters must be callable in every profile, including one
    /// that compiles out both policy features.
    #[tokio::test]
    async fn policy_setters_are_callable_in_every_profile() {
        let server = CountingServer::start(200, None).await;
        let client = Client::new();

        let response = client
            .get(&server.url("/"))
            .unwrap()
            .without_retry()
            .redirect_policy(RedirectPolicy::strict(0))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
        server.shutdown.send(true).unwrap();
    }

    /// With the retry engine enabled, `without_retry()` must suppress the
    /// client-level retry policy exactly as it did before the setter became
    /// profile-independent.
    #[cfg(feature = "logical-retry")]
    #[tokio::test]
    async fn without_retry_suppresses_the_client_retry_policy() {
        use eggfetch_core::RetryPolicy;

        let server = CountingServer::start(503, None).await;
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .backoff_factor(0.0)
            .build();
        let client = Client::builder().retry(policy).build();

        let response = client
            .get(&server.url("/"))
            .unwrap()
            .without_retry()
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 503);
        assert_eq!(
            server.request_count.load(Ordering::SeqCst),
            1,
            "without_retry() must produce exactly one logical attempt"
        );
        server.shutdown.send(true).unwrap();
    }

    /// The order of the two setters must keep its previous meaning: a later
    /// `retry()` call re-enables retries for the request.
    #[cfg(feature = "logical-retry")]
    #[tokio::test]
    async fn retry_after_without_retry_re_enables_retries() {
        use eggfetch_core::RetryPolicy;

        let server = CountingServer::start(503, None).await;
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .backoff_factor(0.0)
            .build();
        let client = Client::builder().retry(policy.clone()).build();

        let response = client
            .get(&server.url("/"))
            .unwrap()
            .without_retry()
            .retry(policy)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 503);
        assert_eq!(
            server.request_count.load(Ordering::SeqCst),
            3,
            "a later retry() call must re-enable retries"
        );
        server.shutdown.send(true).unwrap();
    }

    /// With the redirect loop enabled, a request-level redirect policy must
    /// be honored exactly as before.
    #[cfg(feature = "redirects")]
    #[tokio::test]
    async fn request_redirect_policy_is_honored() {
        let server = CountingServer::start(302, Some("/final".to_owned())).await;
        let client = Client::new();

        let response = client
            .get(&server.url("/start"))
            .unwrap()
            .redirect_policy(RedirectPolicy::new(true, 5))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(server.request_count.load(Ordering::SeqCst), 2);
        server.shutdown.send(true).unwrap();
    }

    /// Without the redirect loop, a 3xx response is returned unchanged and
    /// the stored policy has no effect.
    #[cfg(not(feature = "redirects"))]
    #[tokio::test]
    async fn redirect_policy_is_inert_without_the_redirect_feature() {
        let server = CountingServer::start(302, Some("/final".to_owned())).await;
        let client = Client::new();

        let response = client
            .get(&server.url("/start"))
            .unwrap()
            .redirect_policy(RedirectPolicy::new(true, 5))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 302);
        assert_eq!(
            server.request_count.load(Ordering::SeqCst),
            1,
            "no redirect loop exists without the redirects feature"
        );
        server.shutdown.send(true).unwrap();
    }
}
