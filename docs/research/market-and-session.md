# Research: market prices, card drops and selling

This note covers what the redesigned UI needs from Steam to show card values, session totals and completion estimates, and what quick-sell will need later. It was checked on 2026-09-29. It merges four research reports (prices, drops, selling, UI), and the claims that decisions rest on were checked again against their sources for this note.

**Evidence.** We made only a few live requests. They were spaced out and signed out, and nothing was posted. Raw responses and copies of sources are under `$SP`, a scratch folder outside the repository. That folder is temporary, so the figures that matter are quoted here. Repository paths are relative to the repo root. Short names such as "SEE" or "ASF" are expanded, with versions and dates, under [Sources](#sources).

---

## At a glance

| Question | Answer | § |
| --- | --- | --- |
| Set prices | Signed-in `market/search/render`, one game and border per request, 10 cards a page | 1.1 |
| One card's bid and ask | `market/orderbook` | 1.1 |
| Currency | The wallet's (CM EMsg 5528). Every amount carries its currency, and we never convert | 1.2 |
| Pace | One market queue, 5 s apart. A 429 pauses it for 10 min, doubling up to 60 | 1.3 |
| Value basis | List, net or instant, as the user chooses | 1.4 |
| Which card dropped | Asset ids pushed over CM, described by one `Econ.GetInventoryItemsWithDescriptions#1` call | 2 |
| Time to finish | Gamma–Poisson estimate with a prior of 30 min per drop and an 80% band | 3.1 |
| Value on completion | Session value + Σ (drops left × expected value per drop) | 3.3 |
| Quick-sell | Later. Model assets, money, quotes and listing states now | 4 |

## Where the reports disagreed

| # | Topic | The reports said | Resolution | § |
| --- | --- | --- | --- | --- |
| D1 | Hash names | Prices: build `{appid}-{name}` and retry with suffixes, as Augmented Steam does. Drops and selling: never build one. | Read them instead: `hash_name` from search/render for sets, `market_hash_name` from the inventory for drops. Only drop fallback C has to build one. | 1.1, 2.4 |
| D2 | Which card dropped | UI: diff owned counts on `/gamecards` (and `?border=1`). Drops and selling: CM asset ids. | CM asset ids. The diff has no asset id, so there is nothing to sell, and crafting or trading during the session corrupts it. It is the last fallback. | 2 |
| D3 | Web inventory page size | Selling: `count=2500` (SGI). Drops: 2000. | 2000, which Steam's own page and ASF use. | 2.3 |
| D4 | Requests per drop | Drops: none new. Prices: one order book per drop. | List and net values come from the cached set with no request. The order book is one queued request per new hash, needed only for the instant basis and just before a sale. | 2.2 |
| D5 | Order book signed in or out | Selling, following SEE: signed out. Prices: signed in, falling back to signed out. | Signed in first, because only that is expected to answer in the wallet currency. Fall back to signed out when a signed-in reply is HTML, and keep that reply's `eCurrency`. | 1.2 |
| D6 | Pacing | Prices: one queue for all market calls at 5 s. Selling: a separate limiter with 10 s between listings. SGI: back off 5 s·2ⁿ. | One market queue on top of `community.rs`'s 300 ms gap, with 10 s between `sellitem` calls inside it. A 429 pauses the queue for 10 minutes, because quick retries make a block last longer. | 1.3, 4.5 |
| D7 | Signed-out limits | MMaakkss: search/render refills about 1 per 4.5 s. woctezuma: 25 per 5 min, about 1 per 12 s. | Signed-in reads at 5 s fit every signed-in figure. Signed-out requests go at least 12 s apart. | 1.3 |
| D8 | Requests to price a game | UI: two (normal and foil). Prices: 1–2 (normal). | 1–2 pages per border, because sets over 10 cards need two. That makes 2–4 with foils. Only games with drops left need prices. | 1.1 |
| D9 | Totals with gaps | Prices: `≥ £1.23 · 3 unpriced`. UI: `≈ $4.10 + 3 unpriced`. | Use `≥` for cards actually held, since unpriced ones can only add to the total. Use `≈` for estimates. | 1.5 |
| D10 | Value on completion | Prices: drops left × the set's mean net value. UI: session value + Σ N_g·EV_g, with a foil term. | Use the UI formula, pricing each card on the chosen basis before averaging. Without foil prices the foil term is 0 and the figure says "excl. foils". | 3.3 |
| D11 | Fee change of December 2025 | Selling: it introduced a 1¢ minimum per fee. Prices: USD results are unchanged. | Both are true. The old code already had the minimums. The new code takes the minimum and the rounding step from the wallet and also applies them to the base price. USD results are the same. | 1.4 |

---

## 1. Prices

### 1.1 Where prices come from

**Set prices come from `search/render`.** One request returns up to 10 cards of one game and one border.

```
GET https://steamcommunity.com/market/search/render/
    ?norender=1&appid=753
    &category_753_Game[]=tag_app_{appid}
    &category_753_item_class[]=tag_item_class_2
    &category_753_cardborder[]=tag_cardborder_0      (tag_cardborder_1 for foils)
    &sort_column=name&sort_dir=asc
    &start=0                                         (then 10, 20, …)
```

- `[]` is sent URL-encoded as `%5B%5D`.
- The request is sent signed in, with the cookie steamcards already uses for steamcommunity.com.
- The live checks also sent `query=&search_descriptions=0&count=100` (`$SP/market-live/search620.url`). Steam ignores `count`.

| Field | Use |
| --- | --- |
| `success` | Must be `true`; otherwise treat the lookup as failed |
| `total_count`, `start`, `pagesize` | Pages hold 10, whatever `count` asks for. Repeat with `start += pagesize` while `start < total_count` |
| `results[].hash_name` | The market hash name, e.g. `620-Chell` or `620-Intro (Trading Card)` |
| `results[].name` | The name with any suffix, e.g. `Intro (Trading Card)`, so don't match on it as it stands |
| `results[].sell_price` | Lowest ask: what a buyer pays including fees, in integer hundredths |
| `results[].sell_price_text` | The same price, formatted. It is the only sign of which currency is in use |
| `results[].sell_listings` | Units listed |
| `results[].asset_description.type` | "Portal 2 Trading Card", or "Portal 2 Foil Trading Card" |
| `results[].sale_price_text` | Ignore it. It is undocumented. In all 13 samples it equalled floor(0.97 × `sell_price`), which is not the seller's cut (a 6¢ card nets 4¢) |

**Live results, 2026-09-29:**
- Portal 2's normal cards came back in one page of 8. With no border filter, `total_count` was 16 but only 10 came back (`$SP/market-live/search620*.body`).
- Half-Life 2 has 8 normal cards at 12–16¢ (mean 13.75¢) and 8 foils at $1.26–$3.36 (mean $1.68) (`$SP/card-ui-research/hl2_*.json`).

**Cost.** Sets have 5–15 cards (UI report), and about half a set drops ([Trading Cards FAQ](https://steamcommunity.com/tradingcards/faq)). So pricing a game takes 1–2 requests for normal cards, or 2–4 with foils (D8).

**Matching results to the set.** To match results to the set read from `/gamecards`, use one of these:
- The signed-in `/profiles/{id}/ajaxgetbadgeinfo/{appid}`. It returns `badgedata.rgCards[]{name, owned, markethash}` (InternalSteamWebAPI wiki; the Steam client's badge store reads the same shape). It costs one extra request per game.
- A name comparison after removing a trailing " (Trading Card)", " (Foil)" or " (Foil Trading Card)".

**One card's bid and ask come from `orderbook`.** This is the new market's endpoint, in use since 2026-05. SteamDB's extension (2026-05-12), SEE 7.3.0 (2026-05-14), Augmented Steam (2026-05-13 and 20) and SGI all use it.

```
GET https://steamcommunity.com/market/orderbook?q=Load&qp=[753,"620-Chell"]     (qp URL-encoded)
X-Valve-Request-Type: queryAction
```

Steam's own `FetchQueryAction` sends that header (SteamTracking `c/steamcommunity.com/ssr/DBfcW04x2.js`), and so does SteamDB's `inventory.js`. A signed-out call without the header still got JSON. This reply came from `$SP/market-live/orderbook620chell.body` at 16:43:36Z:

```json
{"data":{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,"eCurrency":2,
 "cBuyOrders":41763,"cSellOrders":4662,"rgCompactBuyOrders":[4,4327,3,37436],
 "rgCompactSellOrders":[5,2,6,3,7,1321,…]}}}
```

| Field | Meaning |
| --- | --- |
| `amtMaxBuyOrder` | Best bid, as the buyer's price. An instant sale fills it, and the seller gets `seller(amtMaxBuyOrder)` (§1.4) |
| `amtMinSellOrder` | Lowest ask |
| `eCurrency` | The currency of every amount in the reply. Read it every time |
| `cBuyOrders`, `cSellOrders` | Total units on each side |
| `rgCompactBuyOrders`, `rgCompactSellOrders` | A flat `[price, quantity, price, quantity, …]` list per price level. The quantities sum to the totals |

- **Accept both reply shapes.** The outer `data` appeared around 2026-08-14 (SEE PR #331).
- **Unknown or localised names** give `{"data":{"success":false}}` (Augmented Steam #2393). Live, `730-SAS` failed and `730-FBI` worked (`$SP/orderbook-730-*.json`, 16:46Z).
- **Some signed-in sessions get HTML** (SEE PR #333, still open).

**`priceoverview` answers in whichever currency is asked for**, as localised strings.

```
GET https://steamcommunity.com/market/priceoverview/?appid=753&currency=1&market_hash_name=620-Chell
→ {"success":true,"lowest_price":"$0.06","volume":"203","median_price":"$0.07"}
  with currency=3, 620-Chell (Foil): {"success":true,"lowest_price":"0,26€","volume":"1","median_price":"0,29€"}
```

- Fields are missing when there are no listings or sales.
- `volume` is 24-hour sales.
- Replies carry `Cache-Control: public,max-age=15` (`$SP/market-live/po620chell_*`).
- It has the smallest measured budget: a 429 on request 21 (MMaakkss). Use it only for a median before a sale (§4.4).

**What no longer works:**
- `itemordershistogram` answers `{"success":104}` (SteamMarketHelper README, 2026-08-17).
- The new listing page (773 KB) no longer contains the `item_nameid` that call needed (`$SP/market-live/listing620chell.body`).

### 1.2 Currency

| Endpoint | Signed out | Signed in |
| --- | --- | --- |
| `search/render` | USD, even from a non-US IP. `currency=3` is ignored (live) | Expected to be the wallet's. **Not tested** |
| `orderbook` | The IP country's currency. `currency=1` is ignored (live, in three reports) | Expected to be the wallet's (SteamDB's extension formats signed-in replies with it). Sometimes HTML |
| `priceoverview` | Whatever `currency` asks for | The same |

The legacy pages always sent `currency=g_rgWalletInfo.wallet_currency` (falling back to 1) and `country=g_strCountryCode` (`market.js` L1423, L2133 and L2257; `economy_v2.js` L3828). The new market, which has been the default since 2026-05-13, sends no currency. No `steamCurrencyId` cookie appears in the community JS we checked, and `steamCountry` holds only the country.

**Rules:**

1. **The display currency is the wallet's.** It arrives as `CMsgClientWalletInfoUpdate.currency` (EMsg 5528, field 3; Protobufs `steammessages_clientserver.proto:313-321`).
   - steamcards drops that message today (`library/steam-api/src/cm.rs:518`).
   - The fallback is `g_rgWalletInfo.wallet_currency` on a signed-in inventory page, which also has the fee parameters (§1.4).
   - An account without a wallet can't sell (§4.1). Show whatever currency its prices come in.
2. **Money is `i64` hundredths plus an ECurrency**, even for whole-unit currencies.
3. **Never add or convert across currencies.** A quote in another currency is shown next to its card in that currency, and counts as unpriced in totals.
4. **search/render has no currency field.** Our rule, building on the prices report's advice to label prices with the currency returned:
   - Format `sell_price` in the wallet currency and compare it with `sell_price_text`.
   - If that doesn't match, try USD.
   - If neither matches, don't use the quote.
5. **Ask for the order book signed in first**, and signed out only when the reply is HTML (D5).
6. **Format amounts as Valve's `v_currencyformat` does** (`global.js:330`):
   - `(v/100).toFixed(2)`.
   - Drop `.00` for whole-unit currencies, except RUB.
   - Use the currency's decimal symbol.
   - Put the symbol before or after, with the currency's separator.
   - EUR writes `,00` as `,--`, and USD adds " USD" outside the US.

ECurrency ids (`g_rgCurrencyData`, `global.js`): 1 USD, 2 GBP, 3 EUR, 4 CHF, 5 RUB, 6 PLN, 7 BRL, 8 JPY, 9 NOK, 10 IDR, 11 MYR, 12 PHP, 13 SGD, 14 THB, 15 VND, 16 KRW, 17 TRY, 18 UAH, 19 MXN, 20 CAD, 21 AUD, 22 NZD, 23 CNY, 24 INR, 25 CLP, 26 PEN, 27 COP, 28 ZAR, 29 HKD, 30 TWD, 31 SAR, 32 AED, 33 SEK, 34 ARS, 35 ILS, 36 BYN, 37 KZT, 38 KWD, 39 QAR, 40 CRC, 41 UYU, 42 BGN, 43 HRK, 44 CZK, 45 DKK, 46 HUF, 47 RON.

### 1.3 Pacing, back-off and cache

| Source (date) | Finding |
| --- | --- |
| MMaakkss, `docs/steam-rate-limits.md` (measured 2026-09) | Each endpoint has its own bucket. priceoverview gave a 429 on request 21. search/render allows a burst of 100 and refills about 1 per 4.5 s. Probing makes a block last longer. |
| SteamMarketHelper README (2026-08) | About 1 request/s brought an IP-wide 429 lasting "several minutes". It uses 1.2 s plus jitter, and a 30 s back-off. |
| SEE `code.user.js:202-231` | 1 s between market requests, plus 1–1.5 s per item. 30–45 s after failures. Stops after 5 errors in 5 minutes. |
| Augmented Steam #1594 (2023), #2365 (2026-08) | A 429 after about 20–30 items at 1–2 s spacing. Card prices became click-to-load. |
| woctezuma `src/api_utils.py` | Signed out: 25 per 5 minutes each for order, search and listing requests. Signed in: 50 a minute for order and search requests, and 25 per 3 minutes for listing pages. |
| ASF wiki, `WebLimiterDelay` | Never more than one request per 300 ms from one IP to one Steam domain (`Configuration.md:352-358`). |

**The plan:**

- **One single-flight market queue** carries every `/market/` request: search/render, orderbook and priceoverview now, and sellitem, mylistings and removelisting later. It sits on top of the site-wide 300 ms gap (`GAP`, `library/steam-api/src/community.rs:18`), not in place of it.
- **Gaps:**
  - signed-in reads: 5 s plus 0–1 s of jitter;
  - signed-out requests: at least 12 s (D7);
  - `sellitem`: at least 10 s after the previous one (§4.5).
- **Budget:** about 60 card sets take about 80 requests, roughly 7 minutes, for normal cards. Foils double that.
- **Refresh order:** games being farmed first, then games with drops left. Games with no drops left don't feed the session or completion figures, so skip them unless a view needs them.
- **On a 429:**
  - Pause every market request for 10 minutes, with no probing.
  - Then probe once. If the probe fails, double the pause, up to 60 minutes.
  - Keep the pause's end time across restarts.
- **Don't reuse quick retries for market calls.** That rules out `community.rs`'s three tries at 500 ms × attempt (`community.rs:19-21, 88-90`) and SGI's 5 s·2ⁿ back-off (`settings.rs:73-81`). Quick retries make a block last longer (MMaakkss).
- **On a 5xx:** retry once after 30 s, then mark the price stale.

| Cached | Fresh for | Then |
| --- | --- | --- |
| Set prices, on disk | 6 h | Shown dimmed with their age, for up to 7 days |
| Order book for a drop this session | 30 min | Fetched again. Always fetched again just before a sale |
| A failed lookup | 24 h | Tried again |

### 1.4 Fees

The fee functions come from Valve's `economy_common.js:107-178` (SteamTracking copy of 2026-07-22). They arrived on 2025-12-03, and the old `CalculateFeeAmount` was gone by 2026-01-07. SEE PR #336 ports them.

Amounts are integer hundredths. The parameters come from `g_rgWalletInfo`. Read them once per session from a signed-in inventory page, or use Valve's defaults, which gave the right answer in every example here.

| Name | Field | Default |
| --- | --- | --- |
| `MIN` | `wallet_market_minimum` | 1 |
| `INC` | `wallet_currency_increment` | 1 |
| `SPCT` (Steam) | `wallet_fee_percent` | 0.05 |
| `PPCT` (publisher) | `wallet_publisher_fee_percent_default` | 0.10 |

```
valid(p):                                              # ToValidMarketPrice
    if p <= MIN: return MIN
    if p <= INC: return INC
    if INC > 1:  return floor(p / INC + 0.5) * INC
    return p

fee(b, pct) = pct > 0 ? valid(floor(b * pct)) : 0      # CalculateFee

# seller gets b  ->  buyer pays                        # GetTotalWithFees
buyer(b) = valid(b) + fee(b, PPCT) + fee(b, SPCT)

# buyer pays t  ->  seller gets                        # GetItemPriceFromTotal
seller(t):
    b = valid(min(floor(t / (1 + PPCT + SPCT)), t - 2 * MIN))
    repeat 3 times:
        x = buyer(b)
        if x == t: return b
        if x < t:  b = b + INC
        else:      b = b - INC; stop repeating
    return max(MIN, b)
```

We re-ran these for this note:

| Buyer pays (¢) | 3 | 4 | 5 | 6 | 8 | 11 | 13 | 23 | 29 | 63 | 100 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Seller gets (¢) | 1 | 2 | 3 | 4 | 6 | 9 | 11 | 20 | 26 | 56 | 88 |

- **Some buyer prices can't occur.** Examples are 22, 33, 44, 45 and 56, and there are 23 such prices under 200¢. `seller(22)` is 19, and 19 lists at 21. So pick a seller amount and show `buyer()` of it. `sellitem` takes the seller amount anyway (§4.1).
- **USD from 1¢ to 4999¢:** the functions match the pre-December-2025 formula and round-trip exactly (prices report).
- **Integer maths is safe:** `b*5/100` and `b/10` equal the float floors up to 10⁶.
- **A live GBP listing** priced at 3p plus 2p of fees came to £0.05.
- **The December 2025 change (D11).** The old code already had per-fee minimums: `wallet_fee_minimum` for Steam's fee and 1 for the publisher's (`$SP/steamtracking/economy_common.26b862bd37.js:101-102`). What changed is that the minimum and the rounding step now come from `wallet_market_minimum` and `wallet_currency_increment`, and apply to the base price as well.
- **The publisher rate has two sources.** Valve's sell dialog prefers `g_rgAppContextData[appid].market_pubfee_rate`, then the wallet default, then 0.10 (`economy_v2.js:4558-4561`). `GetItemPriceFromTotal` uses only the wallet default (`economy_common.js:150`). Keep both in `WalletInfo`. Every card seen used 10%.
- **Work out net per card, then average.** Fees are floored and have minimums, so `seller()` of the average differs from the average of `seller()`. For Half-Life 2 it is 12¢ against 11.75¢.
- **Valve's sell dialog refuses** a seller amount under `MIN`, a buyer price under 3 × `MIN`, or either one above `wallet_trade_max_balance` (`economy_v2.js:4288-4304`).

**Three bases for any value:**

| Basis | Per card | From |
| --- | --- | --- |
| List | The lowest ask: what a buyer pays | `sell_price` or `amtMinSellOrder` |
| Net | `seller(ask)`: what listing at that price pays out | The same |
| Instant | `seller(bid)`: what selling now pays out | `amtMaxBuyOrder` |

Example: Half-Life 2's G-Man on 2026-09-29 had an ask of 11p and a bid of 8p. Net is therefore 9p and instant is 6p (`$SP/card-ui-research/orderbook.json`).

### 1.5 Unknown, pending and stale

| State | When | The card shows | In totals |
| --- | --- | --- | --- |
| Pending | Not fetched yet | A dim `…`, never `0` | Unpriced |
| No market | Marketable, but no listings | "no market" | Unpriced |
| Not marketable | `marketable` is false | `—` | Left out |
| Failed | `success:false`, or a reply we can't parse | `?` | Unpriced; retried after 24 h |
| Stale | Past its time to live | Dimmed, with its age | Included; the total shows the oldest age |
| Other currency | Not the wallet's currency | The amount in its own currency | Unpriced |
| Paused | During a 429 back-off | As before | A banner: "prices paused until 14:32" |

A total of cards held reads `≥ £1.23 · 3 unpriced`. An estimate reads `≈ £15.80` (D9).

---

## 2. Which card dropped

### 2.1 The method

Take each drop's **asset id from what Steam pushes over the CM connection**. Then describe all the new ids with **one CM call, `Econ.GetInventoryItemsWithDescriptions#1`, filtered to those ids**. That call returns what selling needs (asset id, `market_hash_name` and `marketable`) and makes no request to steamcommunity.com.

- **Steam's own site does exactly this.** Item notifications (type 4) carry `{app_id, context_id, asset_id}`. The site describes them with `GetInventoryItemsWithDescriptions {steamid, appid, contextid, get_descriptions, language, filters.assetids: [asset_id]}`.
  - Source: SteamTracking `community/chunk~8f4f68fd6.js` (2026-09-16), near "Item notification missing required attributes".
  - A readable copy is `IL()` in steam-ui-unobfuscated `src/chunk~2dcc5aaf7/655.js` (client build of 2025-03-11).
- **Steam pushes drops by two routes:**

| Route | Message | Seen in |
| --- | --- | --- |
| Item announcements | EMsg 5576, `CMsgClientItemAnnouncements.unseen_items {appid, context_id, asset_id, amount, rtime32_gained, source_appid}` (Protobufs `steammessages_clientserver_2.proto:290-302`) | The Steam client's badge store reads `unseen_items[].source_appid` (`actual_src/stores/47801.js`), which is evidence that Steam fills it in. ASF uses 5576 only as a trigger. steamcards reads only `count_new_items`. |
| Notifications | EMsg 146, `SteamNotificationClient.NotificationsReceived#1`, type 4, app 753, context 6 | xPaw's farmer (`index.ts:492-538`). It needs the "new item" notification turned on (`index.ts:17-22`). |

- **xPaw's farmer does not use 5576.** It stopped counting it on 2024-07-30 (commits c5ebdcd and 41d1bc8). A TODO there mentions resetting the counter "if reaching 100".
- **ASF doesn't use the web inventory for its own items.** It reads its own inventory over CM (`ArchiHandler.GetMyInventoryAsync`, `ArchiHandler.cs:186-333`):
  - 2000 items a page, continuing with `start_assetid = last_assetid`;
  - a retry after 2 s on Busy, DuplicateRequest, Fail, RemoteCallFailed, ServiceUnavailable or Timeout.

  Its `InventoryLimiterDelay` is "not used for fetching our own inventory" (wiki `Configuration.md:203`).
- **The web inventory's limit looks separate and stricter.** Live, it answered the first signed-out request with a 429 (16:40:18Z, a gzipped `null` body, no `Retry-After`). Market requests from the same IP a few minutes later went through (`$SP/inv-check/`).
- **The `/gamecards` diff the UI report proposed is the last fallback** (D2).

### 2.2 Per drop

0. At sign-on, note the session start and send `ClientRequestItemAnnouncements` (EMsg 5577). The unseen ids in the answer are the baseline, not drops.
1. Steam pushes 5576, and perhaps EMsg 146. No request is needed.
   - Keep items with `rtime32_gained` at or after the session start.
   - Dedupe by asset id, because the count stays above 0 until the inventory page is viewed.
2. Make one CM call for all new ids: `Econ.GetInventoryItemsWithDescriptions#1 {steamid, appid: 753, contextid: 6, get_descriptions: true, language: "english", filters: {assetids: […]}}` (Protobufs `steammessages_econ.steamclient.proto:14-32`). If an id comes back in `missing_assets`, or without a description, try once more after 2 s.
3. Keep descriptions tagged `item_class_2`, and record `{game, name, foil, market_hash_name, asset_id, marketable}` (§2.4).
4. Keep `/profiles/{id}/gamecards/{appid}?l=english` as the source of truth for drops left. steamcards already reads it 2 s after new items (`AFTER_NEW_ITEMS`, `component/farming/domain/src/rules.rs:25`).
5. Price the drop. List and net values come from the cached set. The order book is fetched once per new hash, through the queue, for the instant basis (D4).

Each drop costs one CM call, the page steamcards already reads, and at most one queued market request.

### 2.3 Fallbacks

- **A. Drops fell but no asset id arrived.**
  - Make the same CM call unfiltered, with `count` 2000, paging by `start_assetid`.
  - Diff against the asset ids held at session start, and keep new `item_class_2` items.
- **B. The CM call fails.**
  - Make the same diff over the signed-in `GET https://steamcommunity.com/inventory/{steamid}/753/6?l=english&count=2000[&start_assetid=…]`.
  - Leave at least 4 s between requests (ASF's `InventoryLimiterDelay` default, `GlobalConfig.cs:95`), and back off for minutes after a 429.
  - Use a count of 2000 (D3). Steam's own page asks for 75 items, then 2000 a page.
  - A 403 with a `null` body on your own inventory means the session has expired (node-steamcommunity `users.js:599-606`).
  - The response shape comes from ASF `InventoryResponse.cs`, node-steamcommunity and `economy_v2.js`; we haven't seen it live:
    - `assets[] {appid, contextid, assetid, classid, instanceid, amount}`, with ids as strings;
    - `descriptions[] {classid, instanceid, name, type, market_name, market_hash_name, market_fee_app, marketable, tradable, commodity, tags[], owner_actions[]}`, with the flags as 0 or 1;
    - `more_items`, `last_assetid`, `total_inventory_count` and `success`.
- **C. Last resort.** Diff owned counts on `/gamecards/{appid}` and `?border=1`. This gives the name and whether it is a foil, and nothing more: there is nothing to sell. Crafting, trading or selling during the session corrupts it.

### 2.4 Foils and hash names

- **A foil** has the tag `cardborder_1`; `cardborder_0` is a normal card (ASF `InventoryDescription.Type`; SEE `getIsFoilTradingCard`). Its `type` reads "… Foil Trading Card".
- **Hash names usually add " (Foil)"**, as in `620-Chell (Foil)`. Name clashes change the suffix:
  - foils can get " (Foil Trading Card)", as in `620-Intro (Foil Trading Card)` and `220-Gordon & Alyx (Foil Trading Card)`;
  - normal cards can get " (Trading Card)", as in `620-Intro (Trading Card)`, `440-DEMOMAN (Trading Card)` and `730-Anarchist (Trading Card)`.

  These were seen live in `$SP/market-live/search620_all_cur3.body` and `$SP/card-ui-research/`; DEMOMAN comes from the InternalSteamWebAPI wiki.
- **Read hash names; never build them (D1).**
  - search/render gives `hash_name` for the whole set.
  - A drop's description gives its `market_hash_name`.
  - Only fallback C has to build one, and there `success:false` means "unknown name", not "no market".
- **Which game a card belongs to:** take `source_appid`. Failing that, take the `Game` tag `app_N`, then `market_fee_app`, then the number before the dash in the hash name.

### 2.5 What steamcards needs

- **`library/steam-api/src/proto.rs:178`:** `ClientItemAnnouncements` holds only `count_new_items`. Add `unseen_items = 2` with these fields:
  - `appid` u32 = 1
  - `context_id` u64 = 2
  - `asset_id` u64 = 3
  - `amount` u64 = 4
  - `rtime32_gained` fixed32 = 5
  - `source_appid` u32 = 6

  Log it on the next drop to confirm that Steam fills it in for this kind of session.
- **`library/steam-api/src/cm.rs:500-508`** turns 5576 into `Event::NewItems(count)`, which reaches the farmer as `Signal::NewItems` (`component/farming/data/src/play.rs:106`, `component/farming/domain/src/model.rs:92`). Carry the items as well as the count.
- **`cm.rs:518`** drops every other push, including EMsg 146 and 5528.
- **`cm.rs:360`:** `call()` can make the Econ call as it is, once the messages from Protobufs `steammessages_econ.steamclient.proto` are added.

---

## 3. Session and completion estimates

### 3.1 Time to finish

**ASF's estimate** (`CardsFarmer.cs:81-125`) is 30 minutes per card left, plus an hours term:
- Take the games under `HoursUntilCardDrops`, sorted by hours, 32 at a time.
- For each group, add the threshold minus the lowest hours in the group.
- ASF's code calls this "still simplified", and its FAQ calls the result best case, "usually longer" (wiki `FAQ.md:73`).

**What's known about drop timing:**
- Most drops come every 30 minutes, but sometimes nothing arrives "even for 4 hours".
- Restricted accounts wait about 3 hours before the first drop (`FAQ.md:61`, `Performance.md:35-37`).
- Each game's developer sets the playtime per card ([Steamworks](https://partner.steamgames.com/doc/marketing/tradingcards)).

**Instead, learn the rate** with Gamma–Poisson shrinkage, as the UI report proposed:

- **Inputs:** K drops in T hours of farming cards alone (`Mode::Cards`). Time spent building hours, paused, blocked or offline doesn't count, and neither do drops outside T.
- **Account rate:** r = (2 + K) / (1 h + T). This is a prior of 2 drops an hour (ASF's 30 minutes), and a few drops outweigh it.
- **Game rate:** r_g = (2 + k_g) / (2/r + T_g). Games not farmed yet use r.
- **ETA** = Σ N_g / r_g, plus ASF's hours term, where N_g is the game's drops left.
- **80% band:** ETA · e^(±1.28·√(1/N + 1/(2+K))), where N = Σ N_g.
- **Before the second drop**, show "≈ 50h (assuming 30 min/drop)" with no band.
- **Worked example:** 6 drops in 2.5 h, with 40 left and every game at the account rate, gives r ≈ 2.3 an hour, an ETA of about 17.5 h, and a band of about 10.7–28.7 h.

Unlike ASF's constant, this estimate follows whatever the farming logic does. So it will show whether later changes to that logic speed farming up.

### 3.2 Value this session

This is the sum, over this session's drops (§2), of each card's price on the chosen basis (§1.4). It is written `≥ … · n unpriced` (§1.5). A foil is priced from the foil set or from its order book.

### 3.3 Expected value on completion

- **Expected value per drop for game g:** EV_g = (1 − f) · mean(normal_g) + f · mean(foil_g).
  - Price each card on the chosen basis first, then average.
  - Assume drops are spread evenly over the set.
- **f, the foil chance, is unpublished.** Valve's FAQ says "a small chance". The 1% figure is a 2016 reply by a community member, not Valve ([thread](https://steamcommunity.com/groups/tradingcards/discussions/1/412448158147701083/)).
- **Value on completion** ≈ session value + Σ N_g · EV_g.
- **Without foil prices (D10),** f = 0 and the figure says "excl. foils". This is the prices report's "drops left × the set's mean net value".
- **Half-Life 2 on 2026-09-29:**
  - List basis: 0.99 × 13.75¢ + 0.01 × 167.9¢ ≈ 15.3¢ per drop. Foils add about 12%.
  - Net basis: 0.99 × 11.75¢ + 0.01 × 146.5¢ ≈ 13.1¢ per drop.
- **Uncertainty:**
  - Which normal card drops barely matters: Half-Life 2's range is about ±15%.
  - The choice of basis matters most.
  - Foils make a game's value bimodal. Show "≈ $0.55 · 4% foil chance" rather than a ± range.
  - Over a session, P(at least one foil) = 1 − 0.99ⁿ, which is 63% at 100 drops.
- **Value per hour** of a queued game is EV_g × r_g. It gives the queue a "most valuable first" sort, which the original Idle Master had.

---

## 4. Selling later

### 4.1 The sell request

```
POST https://steamcommunity.com/market/sellitem/
Cookie:  steamLoginSecure=…; sessionid=…
Referer: https://steamcommunity.com/profiles/{steamid}/inventory
Body:    sessionid={the cookie's value}&appid=753&contextid=6&assetid={id}&amount=1&price={seller amount, hundredths}
```

- **`price` is what the seller gets per unit** (`economy_v2.js:4364-4378`).
- **The Referer is needed.** Sources:
  - steampy `market.py:123` and stlib `community.py:817` send it;
  - SGI sends `Origin` as well (`market.rs:260-266`);
  - [adambard (2018)](https://adambard.com/blog/programatically-liquidating-my-steam-inventory/) found it "completely necessary".
- **The reply** is `{success, requires_confirmation (0/1), needs_mobile_confirmation, needs_email_confirmation, email_domain}`, or `{success: false, message}` (`economy_v2.js:4431-4496`).
- **Retry later on these messages** (SEE `code.user.js:1274-1282`):
  - "You cannot sell any items until your previous action completes."
  - "There was a problem listing your item…"
  - "…unable to contact the game's item server…"
  - steampy also handles "…already have a listing for this item pending confirmation" (`market.py:126`).
- **Check these first, as Valve's page does** (`economy_v2.js:3915-3929, 4256-4311`):
  - a wallet currency is set and `g_bMarketAllowed` is true;
  - the seller amount is at least `wallet_market_minimum`;
  - the buyer price is at least 3 × that minimum and at most `wallet_trade_max_balance`.
- **Other actions:**
  - `POST /market/removelisting/{id}` cancels a listing (`market.js:40-60, 117`).
  - The 2026 market adds `UpdateSellListingPrice` and `CancelSellListing` under `/market/actions` (SteamTracking `c/steamcommunity.com/ssr/DBEJy_Ja2.js`). `UpdateSellListingPrice` reprices a listing in place and returns `bRequiresConfirmation`. We haven't checked their request format.
  - `/market/mylistings` gives listing ids, and whether each is pending or active (SGI `parse_listing_ids`, SEE `code.user.js:3069-3073`).

### 4.2 Confirmations

- **steamcards can't confirm listings.** Confirming signs `mobileconf/getlist` and `multiajaxop` with the account's `identity_secret` (ASF `ArchiWebHandler.cs:1868, 2073`). The confirmation type is `MarketListing = 3` (SteamKit `enums.steamd`).
- **Often no confirmation is needed.** Valve's [Relaxed Confirmation Requirements](https://steamcommunity.com/groups/community_market/announcements/detail/1705067494681435160) (2018-10-02) say:
  - items that usually sell for under $1 skip confirmation when listed "reasonably within the median price", until "many" cheap items have been listed;
  - rare or valuable items always need it.

  So drops listed one at a time near the market price should mostly go live straight away. Bursts, or prices well under the market, will pile up.
- **Pending listings** wait under "My listings awaiting confirmation" and can't sell, but they can be cancelled. In the Steam app, market listings can be ticked and confirmed together, but trades can't ([a Valve reply, quoted on 2022-10-17](https://steamcommunity.com/discussions/forum/8/5533260453836001644/)). There is no "select all"; check the current app.
- **Accounts on email Steam Guard** need an emailed link for every listing (`email_domain`), and each listing is held for 15 days. Quick-sell isn't practical for them, so turn it off and say why.

### 4.3 Restrictions

These come from search-engine snippets, because the help pages load by script and didn't render. Check the pages themselves before relying on them.

| Rule | Source |
| --- | --- |
| Steam Guard must have been on for 15 days before the market works | Snippet only |
| A password reset locks trading and the market for 5 days | Snippet only |
| A new device is locked for 7 days, unless the mobile authenticator has been on for 7 days. Removing the authenticator locks for 15 days | [451E-96B3-D194-50FC](https://help.steampowered.com/en/faqs/view/451E-96B3-D194-50FC) |
| Listings made without the authenticator on for the last 7 days are held for 15 days | [34A1-EA3F-83ED-54AB](https://help.steampowered.com/en/faqs/view/34A1-EA3F-83ED-54AB) |
| Limited or banned accounts can't use the market | [71D3-35C2-AD96-AA3A](https://help.steampowered.com/en/faqs/view/71D3-35C2-AD96-AA3A) |

- For an account on email Steam Guard, steamcards' QR session probably counts as a new device.
- A `webTradeEligibility` cookie (`allowed`, `steamguard_required_days`, `new_device_cooldown_days`), seen in SEE issue #321 (2026-03-27), might work as a check beforehand. This is not verified.

### 4.4 Pricing

| Strategy | Seller amount | Sells | Used by |
| --- | --- | --- | --- |
| Instant | `seller(bid)` | Now | A SEE button. SGI's default, plus a flat adjustment, within $0.01–10 |
| Undercut | `seller(ask) − INC` | First in line | A SEE button, "lowest listing − 1¢" |
| Match | `seller(ask)` | After older listings at that price | A SEE button |
| The higher of the 12 h average and the lowest listing | Varies | May wait | SEE's default (`code.user.js:3994-3999`) |

**SEE's guards:**
- it skips a lowest listing that has too few units behind it (`code.user.js:443-487`);
- it clamps buyer prices to $0.05–2.50 for cards and $0.15–10 for foils (lines 275-293);
- it never prices below the highest buy order;
- it skips buyer prices of 3¢ or less.

SEE relists by removing a listing, finding the item's new asset id by hash name, and selling again (lines 2853-2925). **Relisting changes the asset id.**

**Default when quick-sell is built: undercut, never below the bid, with instant as an option.**
- The relaxed confirmation rule rewards staying near the median, and the bid can sit well below it. Live, G-Man had a bid of 8p against an 11p ask, and Chell had 4p against 5p.
- A `priceoverview` median in the wallet currency can check a price before listing. That costs one request per sale.

### 4.5 Pacing

- **Selling uses the market queue (§1.3)**, with at least 10 s between `sellitem` calls (SGI).
- **Retry-later messages** pause the queue for 30–45 s.
- **Five 4xx or 429 errors within 5 minutes** stop auto-selling (SEE `code.user.js:200-231, 1335-1395`). So do 401, 403 and eligibility errors.
- **SGI** stops a batch on a "rate limit" message, and doesn't act on the confirmation flags.
- This is how D6 is resolved.

### 4.6 What the domain should hold now

steamcards already has `library::Game` and `Card { name, owned }` (`component/library/domain/src/model.rs:57-114`). It also has `farming::FarmingStatus`, `EventKind::Dropped` and `Signal::NewItems` (`component/farming/domain/src/model.rs`). Quick-sell can be added later without reshaping the model if these types exist first:

| Type | Holds | Why now |
| --- | --- | --- |
| `Money` | `minor: i64`, plus `currency` (an ECurrency) | Every figure in the UI (§1.2) |
| `CardAsset` | `asset_id`, `context_id` (6), `app_id` (the game), `name`, `market_hash_name`, `market_fee_app`, `foil`, `marketable`, `tradable`, `gained_at` | It identifies the drop, and it is exactly what `sellitem` needs |
| `Drop` | When it happened; a `CardAsset`, or just the name and foil flag (fallback C, never sellable); the quote seen at the time | The drop feed and lifetime value |
| `Card` (existing) | Add `market_hash_name` | Matching set prices to the set |
| `PriceQuote` | `ask` and `bid` (buyer prices, as `Option<Money>`), the depth on each side, the source (search, order book or overview), `fetched_at` | The three bases, and staleness |
| `Price` | `Pending`, `Known(PriceQuote)`, `NoMarket`, `NotMarketable`, `Failed { retry_at }` | The states in §1.5 |
| `WalletInfo` | Currency, minimum, increment, Steam and publisher rates, trade maximum | Fees and sell checks |
| `Basis` | `List`, `Net`, `Instant` | The user's toggle |
| `MarketPause` | When the pause ends, and the current step | It must survive restarts |

These are named now and built later:

| Type | Holds |
| --- | --- |
| `SellPolicy` | Strategy, adjustment, floor and ceiling for normal and foil cards, which games are opted in, the gap between listings |
| `Listing` | Listing id, current asset id (relisting changes it), seller amount, state |
| `ListingState` | `Queued → Priced → Submitted`, then `Listed`, `AwaitingConfirmation` (mobile or email), `RetryAt(time)` or `Rejected(message)`. Final states: `Sold`, `Cancelled` or `Removed` |

**Where these live:**
- A `market` component, with `domain`, `data` and `di` like the others under `component/`, can own quotes, wallet info and, later, listings.
- Farming reports drops together with their `CardAsset`s.
- The market requests go in `library/steam-api`, behind the market queue.

---

## 5. What other farmers show

| Farmer | Shows | Missing |
| --- | --- | --- |
| ASF and ASF-ui | Console: "We have a total of {g} games ({c} cards) left to farm (~{t} remaining)..." and "Farming status for {app} ({name}): {n} cards remaining" (`Strings.resx:271, 286`). ASF-ui: three tiles for games, time and cards left, summed across bots, with time as the maximum (`FarmingInfo.vue:3-5`, `bots.js:58-60`). A grid of game header images, with the ones being farmed highlighted (`Games.vue`). | Which card dropped and money. Its time estimate is a constant. |
| Steam Game Idler | Sections "Farming (n)" or "Building playtime (n)", "Up next (n)" and "Completed this session (n)". The active card shows a cover, the name, a bar of initial − remaining, and "N drops remaining" or "1.2h / 3h until farmable". Queue rows read "Waiting its turn". Completed rows give a reason, such as the refund window with a resume date, or skipped (`CardFarming{ProgressView,ActiveCard,GameRow}.tsx`). It refreshes every 5–8 minutes. | An ETA, a drop feed, and value while farming. Prices appear only in its Inventory Manager: a "Fetch price" button per item, and "Sell dupes" and "Sell all" with an "Estimated time" (`src-tauri/src/inventory/market.rs`). |
| Idle Master Extended (archived) | "{n} games left to idle, {m} idle now." and "{n} card drops remaining". The current game's drops left and "hrs on record", and one overall bar (`frmMain.cs:196-216, 577-591`). "Next check mm:ss": 15 minutes, or 5 on the last card (`frmMain.cs:208, 1311`). Statistics for the session and all time (`Statistics.cs`). | An ETA and value |
| Idle Master (original) | Sorted games "most valuable first" using Enhanced Steam's average set prices (`frmMain.cs:80-93`). That host no longer resolves. | — |
| xPaw's farmer | Logs each drop as it lands: "Got a drop for app X, drops remaining: N" (`index.ts:492-530`). | Which card dropped and value |

None of them shows an ETA that learns from real drops, which card dropped, or any money.

**Worth borrowing, set against what the redesign needs:**

| Element | Contents | Covers | From |
| --- | --- | --- | --- |
| Header | "37/112 drops · 5/23 games", an ETA such as "≈ 6h 10m (4h50–7h50)", session value on the chosen basis, "≈ $15.80 at completion" | Overall progress, session total, value on completion | ASF-ui's tiles, IME's bar |
| Now farming | The game; its mode (Cards, or "1.2h / 3h until farmable"); a k/N bar; time on the game; "last drop 12m ago"; minutes per drop; next check as mm:ss; EV per drop | Progress per game | SGI's active card, IME's next check |
| Queue | Drops left, hours, EV per drop, value left, share of the ETA, status (waiting, building hours, or in the refund window until a date). Sortable by value per hour | Progress per game | SGI's "Up next", Idle Master's sort |
| Drop feed | Time, game, card, ★ for a foil, price, running total. Room for a sale column: listed at a price, needs confirmation, or sold | Cards this session, value per card, quick-sell later | xPaw's log |
| Completed | Games finished this session, with reasons | Existing features | SGI |
| Stats | Session and lifetime time, cards, drops per hour, value | — | IME |
| Price footer | Currency, basis, age, unpriced count, back-off state | Being honest about gaps | §1.5 |
| Quick-sell settings, later | Auto-list on or off, strategy, floor and limits, gap between listings, pending confirmations | Quick-sell | SEE, SGI |

---

## 6. Open questions and risks

| # | Question or risk | Why it matters | How to settle it |
| --- | --- | --- | --- |
| 1 | Does Steam fill in `unseen_items` for card drops in a session like steamcards'? Does it depend on the "new item" notification setting? Is there a cap (xPaw's TODO mentions "if reaching 100")? | It is the main route for identifying drops | **Answered live, 2026-09-30: yes.** Asked at sign-on, Steam listed two unseen card drops with `appid` 753, `context_id` 6, `asset_id`, `rtime32_gained` and `source_appid` (the game) all filled in. With nothing unseen, it didn't answer at all, so an answer only counts if it comes promptly. The cap is still unknown |
| 2 | The currency of signed-in search/render is untested | USD set prices can't be added to totals in the wallet currency | Make one signed-in request from a non-USD wallet |
| 3 | Does search/render list a card that has no listings? | It decides whether a missing card is "no market" or a failure | Find a set with an unlisted card |
| 4 | The signed-in order book sometimes returns HTML (SEE PR #333, open) | The fallback answers in the IP country's currency | Check the content type, and count how often it happens |
| 5 | The market endpoints keep moving. The order book dates from 2026-05 and changed shape on 2026-08-14, the histogram is dead, and the fees changed twice in 2025–26 | Parsers can break without warning | Accept both reply shapes. Show failures as `?`, never 0. Test the fee code against the table in §1.4 |
| 6 | Rate limits are undocumented, the measured figures disagree, and SteamMarketHelper describes its 429 as IP-wide | A market block might also stop the badge pages that farming needs | Give farming's requests priority. Stop market calls at the first 429 |
| 7 | The foil rate (1%, from a 2016 community reply) and the assumption that drops are spread evenly | Expected value (foils add about 12% for Half-Life 2) | Count foils across sessions |
| 8 | Valve doesn't define "reasonably within the median" or "many" | Quick-sell could pile up pending listings | List a few by hand first, and show the pending count |
| 9 | The restrictions come from snippets, and a QR session may count as a new device | Whether quick-sell is available at all | Read the help pages in a browser, and try `webTradeEligibility` |
| 10 | The request format of `UpdateSellListingPrice` and `CancelSellListing` is unknown | Repricing listings | Read `ssr/DBEJy_Ja2.js` when it's needed |
| 11 | The signed-out web inventory answered with a 429 straight away | Fallback B may fail | Try A before B |
| 12 | Not researched: what Steam's terms say about automated listing | Quick-sell | Read the Steam Subscriber Agreement and the market FAQ before building it |

---

## Sources

| Short name | What | Version and date |
| --- | --- | --- |
| Live: prices | 6 signed-out GETs, 16:42–16:45Z on 2026-09-29, at least 6 s apart, from a non-US IP; no 429 | `$SP/market-live/` |
| Live: UI | 5 signed-out requests from about 16:40Z, at least 14 s apart; no 429 | `$SP/card-ui-research/`, `$SP/steam-requests-cardui.log` |
| Live: inventory | 2 signed-out GETs at 16:40:12Z and 16:40:18Z; the second got a 429 | `$SP/inv-check/` |
| Live: selling | 6 signed-out GETs, at least 5 s apart; no 429, nothing posted | `$SP/orderbook-730-*.json` |
| Valve JS | `economy_common.js`, `economy_v2.js`, `market.js`, `global.js` and the new market's `ssr/` chunks, via [SteamTracking](https://github.com/SteamTracking/SteamTracking) | JS of 2026-07-22; community chunk of 2026-09-16; `$SP/steamtracking/`, `$SP/st_mine/` |
| Protobufs | [SteamDatabase/Protobufs](https://github.com/SteamDatabase/Protobufs) | 623ebf5, 2026-09-25 |
| steam-ui-unobfuscated | [ricewind012/steam-ui-unobfuscated](https://github.com/ricewind012/steam-ui-unobfuscated) | Client build of 2025-03-11; `$SP/steamui/` |
| ASF | [JustArchiNET/ArchiSteamFarm](https://github.com/JustArchiNET/ArchiSteamFarm), its [wiki](https://github.com/JustArchiNET/ArchiSteamFarm/wiki) and [ASF-ui](https://github.com/JustArchiNET/ASF-ui) | d02cb6e, 2026-09-28; wiki 130cae5, 2026-09-20; ASF-ui c154f69, 2026-09-29 |
| SEE | [Steam Economy Enhancer](https://github.com/Nuklon/Steam-Economy-Enhancer), MIT | v7.3.2, d873ca3, 2026-08-17 |
| SGI | [Steam Game Idler](https://github.com/zevnda/steam-game-idler), Elastic-2.0 | 6.2.7, e2c374f3, 2026-09-25 |
| xPaw's farmer | [xPaw/Steam-Card-Farmer](https://github.com/xPaw/Steam-Card-Farmer) | 360bafb, 2026-09-19 |
| IME and Idle Master | [JonasNilson/idle_master_extended](https://github.com/JonasNilson/idle_master_extended) and [jshackles/idle_master](https://github.com/jshackles/idle_master) | 065273c, 2026-04-28 (archived); 54c2926, 2018-01-03 |
| node-steamcommunity | [DoctorMcKay/node-steamcommunity](https://github.com/DoctorMcKay/node-steamcommunity) | fa8d897, 2026-07-13 |
| steampy and stlib | [bukson/steampy](https://github.com/bukson/steampy) and [calendulish/stlib](https://github.com/calendulish/stlib) | 5a77848, 2024-12-23; 647c1aa, 2024-11-04 |
| woctezuma | [woctezuma/steam-market](https://github.com/woctezuma/steam-market), `src/api_utils.py` | 7aa7f57, 2026-07-14 |
| Augmented Steam | [IsThereAnyDeal/AugmentedSteam](https://github.com/IsThereAnyDeal/AugmentedSteam): issues #1594, #2365 and #2393; `CardLowestPrice.svelte`; its server's `src/Data/Updaters/Market/MarketCrawler.php` | 2023 to 2026-08; `$SP/augsteam/` |
| SteamDB extension | [SteamDatabase/BrowserExtension](https://github.com/SteamDatabase/BrowserExtension), `inventory.js` | 2026-05-12; `$SP/steamdb-ext/` |
| MMaakkss | MMaakkss/cs-skins-prices-parser, `docs/steam-rate-limits.md` | Measured 2026-09 |
| SteamMarketHelper | federicogiorgi/SteamMarketHelper, README | Verified 2026-08-17 |
| InternalSteamWebAPI | [Revadike/InternalSteamWebAPI wiki](https://github.com/Revadike/InternalSteamWebAPI/wiki) | Read 2026-09-29 |
| SteamKit | [SteamRE/SteamKit](https://github.com/SteamRE/SteamKit), `enums.steamd` | Read 2026-09-29 |
| Valve pages | The [Trading Cards FAQ](https://steamcommunity.com/tradingcards/faq), [Steamworks: trading cards](https://partner.steamgames.com/doc/marketing/tradingcards), and the announcement, help pages and threads linked above | Read 2026-09-29 |
