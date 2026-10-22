//! Lean single-hop dispatch for profiles without redirect following.
//!
//! Used when the `redirects` feature is disabled (regardless of whether
//! `logical-retry` is enabled: the retry loop calls [`send_lean`] as its
//! inner attempt, and the client calls it directly when both policy
//! features are absent).
//!
//! The lean path dispatches once under the outer total deadline, returns
//! 3xx responses as ordinary responses without a second hop, constructs no
//! redirect history, and needs no cross-origin credential reconstruction.
//! Request timeout/body-limit/failure semantics are unchanged from the
//! full pipeline's first hop.
//!
//! Preparation itself is not duplicated here: the header merge, timeout
//! merge, hop construction, cookie injection, auth resolution, and the
//! `Set-Cookie` jar update all live in the shared [`super::hop`] builder,
//! so the lean path and the redirect path differ only in what they do
//! after preparation.

use crate::client::Client;
use crate::error::Result;
use crate::request::Request;
use crate::response::Response;

use super::hop;

/// Send one request without retry or redirect orchestration.
///
/// Preparation is delegated to the shared builder in [`super::hop`], so the
/// lean path and the redirect path cannot diverge except for policy-loop
/// behavior itself.
pub(crate) async fn send_lean(client: &Client, request: Request) -> Result<Response> {
    let params = hop::prepare_first_hop(client, request);
    let timeout = params.timeout;
    let hop = hop::build_hop_request(client, params)?;

    let response = client.send_single_request(hop, &timeout).await?;

    hop::update_cookie_jar(client, &response);

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::hop::test_support;
    use super::*;

    /// The lean path must prepare exactly the hop the shared builder
    /// specifies. The redirect path asserts the same expectation in its own
    /// build, which is what proves the two paths cannot diverge.
    #[tokio::test]
    async fn lean_path_prepares_the_expected_hop() {
        for with_request_auth in [true, false] {
            test_support::clear_recorded();
            let client = test_support::scenario_client();
            let request = test_support::scenario_request(&client, with_request_auth);

            // The dispatch itself fails (nothing listens on the port); the
            // prepared hop is captured before the transport is reached.
            let _ = send_lean(&client, request).await;

            let captured = test_support::take_recorded().expect("a prepared hop was captured");
            assert_eq!(
                captured,
                test_support::expected_snapshot(with_request_auth),
                "the lean path's prepared hop must match the shared expectation"
            );
        }
    }

    /// The `Set-Cookie` jar update must leave the same jar state the
    /// redirect path leaves.
    #[cfg(feature = "cookies")]
    #[tokio::test]
    async fn lean_path_updates_the_cookie_jar() {
        use crate::cookie::CookieJar;

        let (url, server) = test_support::spawn_set_cookie_server().await;
        let client = Client::builder().cookie_jar(CookieJar::new()).build();

        let response = client.get(&url).unwrap().send().await.expect("response");
        assert_eq!(response.status(), http::StatusCode::OK);
        server.await.expect("server task");

        assert_eq!(
            client
                .cookies()
                .cookies_for_url(&url::Url::parse(&url).expect("valid URL")),
            Some(test_support::EXPECTED_COOKIE_HEADER.to_owned()),
            "the lean path must leave the same jar state as the redirect path"
        );
    }
}
