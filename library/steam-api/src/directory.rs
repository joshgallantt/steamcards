//! Where Steam's CM servers are: Steam's Web API lists them, best first for
//! where this computer is.

use anyhow::anyhow;
use serde::Deserialize;

/// The WebSocket URLs of CM servers, best first.
pub(crate) async fn websocket_servers(
    http: &reqwest::Client,
    api: &str,
) -> anyhow::Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Answer {
        response: Listing,
    }
    #[derive(Deserialize)]
    struct Listing {
        #[serde(default)]
        serverlist: Vec<Server>,
    }
    #[derive(Deserialize)]
    struct Server {
        endpoint: String,
        #[serde(rename = "type")]
        kind: String,
    }

    let answer: Answer = http
        .get(format!("{api}/ISteamDirectory/GetCMListForConnect/v1/"))
        .query(&[("cellid", "0")])
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| anyhow!("couldn't reach Steam ({e})"))?
        .json()
        .await
        .map_err(|e| anyhow!("Steam's server list doesn't read ({e})"))?;
    let servers: Vec<String> = answer
        .response
        .serverlist
        .into_iter()
        .filter(|s| s.kind == "websockets")
        .map(|s| format!("wss://{}/cmsocket/", s.endpoint))
        .collect();
    if servers.is_empty() {
        return Err(anyhow!("Steam listed no servers to connect to"));
    }
    Ok(servers)
}

#[cfg(test)]
mod tests {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    use super::*;

    #[tokio::test]
    async fn lists_the_websocket_servers_in_steams_order() {
        let steam = MockServer::start().await;
        // Trimmed from what the Web API answered, live.
        let body = serde_json::json!({"response": {"serverlist": [
            {"endpoint": "ext1-syd1.steamserver.net:27017", "legacy_endpoint": "103.10.125.154:27017", "type": "netfilter", "dc": "syd1", "realm": "steamglobal", "load": 18, "wtd_load": 16.6},
            {"endpoint": "ext1-syd1.steamserver.net:443", "legacy_endpoint": "103.10.125.154:27033", "type": "websockets", "dc": "syd1", "realm": "steamglobal", "load": 18, "wtd_load": 16.6},
            {"endpoint": "ext2-syd1.steamserver.net:27037", "legacy_endpoint": "103.10.125.155:27037", "type": "websockets", "dc": "syd1", "realm": "steamglobal", "load": 20, "wtd_load": 18.1}
        ], "success": true, "message": ""}});
        Mock::given(method("GET"))
            .and(path("/ISteamDirectory/GetCMListForConnect/v1/"))
            .and(query_param("cellid", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&steam)
            .await;

        let servers = websocket_servers(&reqwest::Client::new(), &steam.uri())
            .await
            .unwrap();
        assert_eq!(
            servers,
            [
                "wss://ext1-syd1.steamserver.net:443/cmsocket/",
                "wss://ext2-syd1.steamserver.net:27037/cmsocket/",
            ]
        );
    }

    #[tokio::test]
    async fn an_outage_says_so() {
        let steam = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&steam)
            .await;
        let e = websocket_servers(&reqwest::Client::new(), &steam.uri())
            .await
            .unwrap_err();
        assert!(e.to_string().contains("couldn't reach Steam"), "{e}");
    }
}
