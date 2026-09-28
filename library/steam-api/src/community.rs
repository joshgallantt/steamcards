//! steamcommunity.com, where the badge pages are. It takes the same sign-in
//! as the Steam client, as a cookie: `steamLoginSecure`, the account's Steam
//! ID and a token for the site (see `auth::web_token`). No web sign-in page
//! is involved, as ASF's `ArchiWebHandler.Init` does it.
//!
//! Requests keep a polite gap between them, and a busy site is asked again a
//! couple of times before giving up.

use std::time::Duration;

use anyhow::{anyhow, bail};
use debug_log::DebugLog;
use rand::RngCore;
use reqwest::{StatusCode, header};
use tokio::{sync::Mutex, time::Instant};

/// The gap between requests: ASF's `WebLimiterDelay`.
const GAP: Duration = Duration::from_millis(300);
const TRIES: u32 = 3;
/// How long to wait before asking a busy site again, times the attempt.
const BACK_OFF: Duration = Duration::from_millis(500);

/// Who pages are fetched as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebLogin {
    pub(crate) steam_id: u64,
    pub(crate) access_token: String,
}

pub(crate) struct Community {
    http: reqwest::Client,
    base: String,
    /// The site's anti-forgery cookie: random, as a browser's would be.
    session_id: String,
    /// When the last request went.
    last: Mutex<Option<Instant>>,
    log: DebugLog,
}

impl Community {
    pub(crate) fn new(http: reqwest::Client, base: String, log: DebugLog) -> Self {
        let mut id = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut id);
        Self {
            http,
            base,
            session_id: hex::encode(id),
            last: Mutex::new(None),
            log,
        }
    }

    /// The page at `path` (with its query), fetched as `who`.
    pub(crate) async fn page(&self, path: &str, who: &WebLogin) -> anyhow::Result<String> {
        let url = format!("{}{path}", self.base);
        let cookie = format!(
            "steamLoginSecure={}%7C%7C{}; sessionid={}; Steam_Language=english",
            who.steam_id, who.access_token, self.session_id
        );
        let mut busy = None;
        for attempt in 1..=TRIES {
            self.wait_turn().await;
            let answer = self
                .http
                .get(&url)
                .header(header::COOKIE, &cookie)
                .send()
                .await;
            match answer {
                Ok(r) if r.status().is_success() => {
                    return r
                        .text()
                        .await
                        .map_err(|e| anyhow!("steamcommunity.com's page didn't arrive ({e})"));
                }
                Ok(r)
                    if r.status() == StatusCode::TOO_MANY_REQUESTS
                        || r.status().is_server_error() =>
                {
                    busy = Some(r.status().to_string());
                }
                Ok(r) => bail!("steamcommunity.com said {} for {path}", r.status()),
                Err(e) => busy = Some(e.to_string()),
            }
            let why = busy.as_deref().unwrap_or_default();
            self.log
                .line(&format!("{path}: {why}, attempt {attempt} of {TRIES}"));
            if attempt < TRIES {
                tokio::time::sleep(BACK_OFF * attempt).await;
            }
        }
        Err(anyhow!(
            "steamcommunity.com didn't answer ({})",
            busy.unwrap_or_default()
        ))
    }

    /// Waits until the gap since the last request has passed.
    async fn wait_turn(&self) {
        let mut last = self.last.lock().await;
        if let Some(at) = *last {
            tokio::time::sleep_until(at + GAP).await;
        }
        *last = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header_regex, method, path},
    };

    use super::*;

    fn who() -> WebLogin {
        WebLogin {
            steam_id: 76_561_197_960_287_930,
            access_token: "eyJ.access.token".into(),
        }
    }

    #[tokio::test]
    async fn pages_come_signed_in_as_the_account() {
        let site = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/profiles/76561197960287930/badges"))
            .and(header_regex(
                "cookie",
                r"^steamLoginSecure=76561197960287930%7C%7CeyJ\.access\.token; sessionid=[0-9a-f]{24}; Steam_Language=english$",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>badges</html>"))
            .mount(&site)
            .await;
        let community = Community::new(reqwest::Client::new(), site.uri(), DebugLog::off());
        let page = community
            .page("/profiles/76561197960287930/badges?l=english&p=1", &who())
            .await
            .unwrap();
        assert_eq!(page, "<html>badges</html>");
    }

    #[tokio::test]
    async fn a_busy_site_is_asked_again() {
        let site = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&site)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&site)
            .await;
        let community = Community::new(reqwest::Client::new(), site.uri(), DebugLog::off());
        assert_eq!(community.page("/x", &who()).await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn requests_keep_their_distance() {
        let site = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&site)
            .await;
        let community = Community::new(reqwest::Client::new(), site.uri(), DebugLog::off());
        let started = std::time::Instant::now();
        for _ in 0..3 {
            community.page("/x", &who()).await.unwrap();
        }
        assert!(started.elapsed() >= GAP * 2, "two gaps between three pages");
    }

    #[tokio::test]
    async fn a_page_that_isnt_there_says_so() {
        let site = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&site)
            .await;
        let community = Community::new(reqwest::Client::new(), site.uri(), DebugLog::off());
        let e = community.page("/gamecards/1", &who()).await.unwrap_err();
        assert!(e.to_string().contains("404"), "{e}");
    }
}
