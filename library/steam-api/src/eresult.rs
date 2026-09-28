//! `EResult`: how Steam says how something went. The values are SteamKit's
//! (`Resources/SteamLanguage/eresult.steamd`); only the ones steamcards acts
//! on, or explains, are here.

use std::fmt;

/// An `EResult` Steam sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EResult(pub i32);

impl EResult {
    pub const OK: EResult = EResult(1);
    pub const FAIL: EResult = EResult(2);
    pub const INVALID_PASSWORD: EResult = EResult(5);
    pub const LOGGED_IN_ELSEWHERE: EResult = EResult(6);
    pub const FILE_NOT_FOUND: EResult = EResult(9);
    pub const BUSY: EResult = EResult(10);
    pub const ACCESS_DENIED: EResult = EResult(15);
    pub const TIMEOUT: EResult = EResult(16);
    pub const BANNED: EResult = EResult(17);
    pub const ACCOUNT_NOT_FOUND: EResult = EResult(18);
    pub const SERVICE_UNAVAILABLE: EResult = EResult(20);
    pub const REVOKED: EResult = EResult(26);
    pub const EXPIRED: EResult = EResult(27);
    pub const LOGON_SESSION_REPLACED: EResult = EResult(34);
    pub const ACCOUNT_DISABLED: EResult = EResult(43);
    pub const TRY_ANOTHER_CM: EResult = EResult(48);
    pub const SUSPENDED: EResult = EResult(51);
    pub const ACCOUNT_LOCKED_DOWN: EResult = EResult(73);
    pub const RATE_LIMIT_EXCEEDED: EResult = EResult(84);
    pub const ACCOUNT_LOGIN_DENIED_THROTTLE: EResult = EResult(87);
    pub const INVALID_SIGNATURE: EResult = EResult(121);

    /// Absent means fail, as Steam's .proto files default it.
    pub(crate) fn of(sent: Option<i32>) -> Self {
        EResult(sent.unwrap_or(Self::FAIL.0))
    }

    pub fn is_ok(self) -> bool {
        self == Self::OK
    }

    /// Steam no longer takes the saved sign-in: signing in again is the only
    /// way on.
    pub fn is_sign_in_rejected(self) -> bool {
        [
            Self::INVALID_PASSWORD,
            Self::ACCESS_DENIED,
            Self::REVOKED,
            Self::EXPIRED,
            Self::INVALID_SIGNATURE,
            Self::ACCOUNT_NOT_FOUND,
            Self::ACCOUNT_DISABLED,
            Self::BANNED,
            Self::SUSPENDED,
            Self::ACCOUNT_LOCKED_DOWN,
        ]
        .contains(&self)
    }

    /// Steam is busy or limiting sign-ins: trying again later may work.
    pub fn is_temporary(self) -> bool {
        [
            Self::BUSY,
            Self::TIMEOUT,
            Self::SERVICE_UNAVAILABLE,
            Self::TRY_ANOTHER_CM,
            Self::RATE_LIMIT_EXCEEDED,
            Self::ACCOUNT_LOGIN_DENIED_THROTTLE,
        ]
        .contains(&self)
    }

    fn name(self) -> Option<&'static str> {
        Some(match self {
            Self::OK => "OK",
            Self::FAIL => "Fail",
            Self::INVALID_PASSWORD => "InvalidPassword",
            Self::LOGGED_IN_ELSEWHERE => "LoggedInElsewhere",
            Self::FILE_NOT_FOUND => "FileNotFound",
            Self::BUSY => "Busy",
            Self::ACCESS_DENIED => "AccessDenied",
            Self::TIMEOUT => "Timeout",
            Self::BANNED => "Banned",
            Self::ACCOUNT_NOT_FOUND => "AccountNotFound",
            Self::SERVICE_UNAVAILABLE => "ServiceUnavailable",
            Self::REVOKED => "Revoked",
            Self::EXPIRED => "Expired",
            Self::LOGON_SESSION_REPLACED => "LogonSessionReplaced",
            Self::ACCOUNT_DISABLED => "AccountDisabled",
            Self::TRY_ANOTHER_CM => "TryAnotherCM",
            Self::SUSPENDED => "Suspended",
            Self::ACCOUNT_LOCKED_DOWN => "AccountLockedDown",
            Self::RATE_LIMIT_EXCEEDED => "RateLimitExceeded",
            Self::ACCOUNT_LOGIN_DENIED_THROTTLE => "AccountLoginDeniedThrottle",
            Self::INVALID_SIGNATURE => "InvalidSignature",
            _ => return None,
        })
    }
}

impl fmt::Display for EResult {
    /// "AccessDenied (15)", or "EResult 99" for one without a name here.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} ({})", self.0),
            None => write!(f, "EResult {}", self.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_it_is() {
        assert_eq!(EResult::ACCESS_DENIED.to_string(), "AccessDenied (15)");
        assert_eq!(EResult(999).to_string(), "EResult 999");
        assert_eq!(EResult::of(None), EResult::FAIL, "absent is a failure");
        assert!(EResult::of(Some(1)).is_ok());
    }

    #[test]
    fn a_rejected_sign_in_is_told_from_a_busy_steam() {
        assert!(EResult::ACCESS_DENIED.is_sign_in_rejected());
        assert!(EResult::EXPIRED.is_sign_in_rejected());
        assert!(!EResult::TRY_ANOTHER_CM.is_sign_in_rejected());
        assert!(EResult::TRY_ANOTHER_CM.is_temporary());
        assert!(!EResult::ACCESS_DENIED.is_temporary());
    }
}
