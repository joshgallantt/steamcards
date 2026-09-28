//! What a Steam token says about itself. Steam's tokens are JWTs: the middle
//! part is base64 JSON naming the account (`sub`) and when the token stops
//! working (`exp`). Nothing here checks a signature; Steam does that.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

/// Who a token is for, and until when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenInfo {
    pub steam_id: u64,
    /// Seconds since the Unix epoch.
    pub expires_at: i64,
}

/// Reads a token's claims; `None` when it isn't a token Steam made.
pub fn read(token: &str) -> Option<TokenInfo> {
    #[derive(Deserialize)]
    struct Claims {
        sub: String,
        exp: i64,
    }
    let payload = token.split('.').nth(1)?;
    let json = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Claims = serde_json::from_slice(&json).ok()?;
    Some(TokenInfo {
        steam_id: claims.sub.parse().ok()?,
        expires_at: claims.exp,
    })
}

/// A token shaped like Steam's, for tests: `steam_id`'s, expiring at
/// `expires_at`, with no real signature.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn fake(steam_id: u64, expires_at: i64) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"typ":"JWT","alg":"EdDSA"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "iss": "steam",
            "sub": steam_id.to_string(),
            "aud": ["client", "web"],
            "exp": expires_at,
        })
        .to_string(),
    );
    format!("{header}.{claims}.c2lnbmF0dXJl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_says_whose_it_is_and_until_when() {
        let t = fake(76_561_197_960_287_930, 1_790_000_000);
        assert_eq!(
            read(&t),
            Some(TokenInfo {
                steam_id: 76_561_197_960_287_930,
                expires_at: 1_790_000_000,
            })
        );
    }

    #[test]
    fn anything_else_reads_as_nothing() {
        assert_eq!(read(""), None);
        assert_eq!(read("not.a.token"), None);
        assert_eq!(read("a.eyJzdWIiOjF9.c"), None, "sub isn't a string");
    }
}
