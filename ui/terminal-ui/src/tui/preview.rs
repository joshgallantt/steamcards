// Renders every screen the spec mocks into an in-memory terminal, from the
// spec's data set, at the mockup's size, so layouts can be checked (and
// eyeballed) without touching a real account:
//
//   cargo test -p terminal-ui previews -- --nocapture
//
// The golden tests hold each screen to its mockup; this prints them all, each
// saying whether it reads as its mockup does.

use super::{fixtures, golden};

#[tokio::test]
async fn previews() {
    for m in golden::mockups() {
        let Some(mut app) = fixtures::for_mockup(&m.title) else {
            continue;
        };
        let screen = golden::screen_text(&golden::render(&mut app, m.width, m.height));
        let drawn = if screen == m.rows {
            "as the spec draws it"
        } else {
            "not yet as the spec draws it"
        };
        println!(
            "\n━━━━ {} {}×{} · {drawn} ━━━━\n{}",
            m.title,
            m.width,
            m.height,
            screen.join("\n")
        );
        assert_eq!(screen.len(), usize::from(m.height), "{}", m.title);
        if m.title.starts_with("onboarding, welcome") {
            // Today's Welcome cuts its notice mid-sentence at 60 × 16; this
            // one keeps it whole, and how to agree to it (§8.3).
            let shown = screen.join(" ");
            for words in [
                "steamcards is unofficial: Valve doesn't make or",
                "support it. Farming cards is at your own risk.",
                "Press [enter] to agree and begin.",
            ] {
                assert!(shown.contains(words), "{words:?} isn't on screen");
            }
        }
    }
}

/// Every state the spec mocks, at the sizes it mocks and either side of
/// each size class's edges: nothing panics, so nothing is wider than its
/// room, and every row is drawn.
#[tokio::test]
async fn every_state_renders_at_every_size_class() {
    let states = [
        fixtures::farming_alone as fn() -> super::App,
        fixtures::queue_at_its_end,
        fixtures::building_hours,
        fixtures::first_minutes,
        fixtures::reading_badges,
        fixtures::waiting_for_hades,
        fixtures::paused,
        fixtures::sign_in_expired,
        fixtures::reconnecting,
        fixtures::nothing_to_farm,
        fixtures::prices_paused,
    ];
    for state in states {
        for (w, h) in [
            (240, 70),
            (217, 40),
            (216, 40),
            (209, 49),
            (200, 50),
            (200, 40),
            (199, 50),
            (240, 39),
            (146, 40),
            (120, 36),
            (120, 35),
            (120, 30),
            (100, 26),
            (99, 26),
            (100, 25),
            (80, 24),
            (72, 20),
            (71, 20),
            (72, 19),
            (72, 17),
            (60, 16),
            (59, 16),
            (50, 12),
        ] {
            let mut app = state();
            let screen = golden::screen_text(&golden::render(&mut app, w, h));
            assert_eq!(screen.len(), usize::from(h));
        }
    }
}
