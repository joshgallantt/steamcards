//! What the account holds, as the Steam client knows it: its licences, how
//! and when each was got, which apps each package holds, and the games it
//! marked private.
//!
//! Steam lists the licences to a session as it signs on. Which apps a
//! package holds comes from Steam's product info (PICS), asked about each
//! package with the access token its licence carries, as SteamKit and ASF
//! ask. The private games come from `AccountPrivateApps.GetPrivateAppList`,
//! as ASF asks.

use std::collections::HashMap;

use crate::{
    cm::Connection,
    keyvalues,
    proto::{GetPrivateAppListRequest, GetPrivateAppListResponse, method},
};

/// A licence the account holds: a package, and how and when it was got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Licence {
    pub package_id: u32,
    /// When the account got it, in seconds since 1970.
    pub got_at: u32,
    /// How it was paid for, as an `EPaymentMethod`: 1 is a product key.
    pub payment_method: u32,
    /// As `ELicenseFlags`.
    pub flags: u32,
    /// What asking Steam about its package takes.
    pub access_token: u64,
}

impl Licence {
    /// Whether it was bought in a way Steam refunds: paid for, not a product
    /// key, a free licence or a hardware promotion, and the account's own,
    /// not borrowed from family. As ASF tells (`Bot.IsRefundable`).
    pub fn is_purchase(&self) -> bool {
        const NONE: u32 = 0;
        const ACTIVATION_CODE: u32 = 1;
        const HARDWARE_PROMO: u32 = 16;
        // A payment method on its own, and a flag on others.
        const COMPLIMENTARY: u32 = 1024;
        const BORROWED: u32 = 0x4000;
        !matches!(self.payment_method, NONE | ACTIVATION_CODE | HARDWARE_PROMO)
            && self.payment_method & COMPLIMENTARY == 0
            && self.flags & BORROWED == 0
    }
}

/// The games the signed-on account marked private, by app ID.
pub(crate) async fn private_apps(conn: &Connection) -> anyhow::Result<Vec<u32>> {
    let answer: GetPrivateAppListResponse = conn
        .call(method::GET_PRIVATE_APPS, &GetPrivateAppListRequest {})
        .await?;
    Ok(answer
        .private_apps
        .map(|list| list.appids)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| u32::try_from(id).ok())
        .collect())
}

/// The apps each of these packages holds, by package ID: `(package ID,
/// access token)` each, as the account's licences give them. A package
/// Steam says nothing readable about is left out.
pub(crate) async fn package_apps(
    conn: &Connection,
    packages: &[(u32, u64)],
) -> anyhow::Result<HashMap<u32, Vec<u32>>> {
    let told = conn.package_info(packages).await?;
    Ok(told
        .into_iter()
        .filter_map(|p| {
            let apps = keyvalues::package_apps(p.buffer.as_deref()?).ok()?;
            Some((p.packageid?, apps))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn got(payment_method: u32, flags: u32) -> Licence {
        Licence {
            package_id: 7,
            got_at: 1_790_000_000,
            payment_method,
            flags,
            access_token: 0,
        }
    }

    #[test]
    fn only_a_purchase_steam_refunds_is_a_purchase() {
        const CREDIT_CARD: u32 = 2;
        const PAYPAL: u32 = 4;
        const WALLET: u32 = 128;
        const GIFT: u32 = 8;
        for bought in [CREDIT_CARD, PAYPAL, WALLET, GIFT] {
            assert!(got(bought, 0).is_purchase(), "{bought}");
        }
        assert!(!got(1, 0).is_purchase(), "a product key");
        assert!(!got(1024, 0).is_purchase(), "free");
        assert!(
            !got(1024 | 2, 0).is_purchase(),
            "free, with a payment method"
        );
        assert!(!got(16, 0).is_purchase(), "a hardware promotion");
        assert!(!got(0, 0).is_purchase(), "no payment method");
        assert!(
            !got(CREDIT_CARD, 0x4000).is_purchase(),
            "borrowed from family"
        );
    }
}
