use std::{future::Future, sync::Mutex, time::Duration};

use anyhow::bail;
use card::{Lookup, MarketPause};
use chrono::{TimeDelta, Utc};
use reqwest::StatusCode;
use steam_api::Reply;
use tokio::time::Instant;

use crate::market::MarketPace;

/// The market's requests, one at a time, at the market's pace.
pub(crate) struct MarketQueue {
    pace: MarketPace,
    /// Held for the whole of a request, waits and all: one at a time. When
    /// the last one finished.
    turn: tokio::sync::Mutex<Option<Instant>>,
    /// Steam's pause, when there is one: its end on tokio's clock, and on
    /// the system's as it was when it began. Past its end, it lasts until a
    /// request gets through.
    pause: Mutex<Option<(Instant, MarketPause)>>,
}

impl MarketQueue {
    pub(crate) fn new(pace: MarketPace) -> Self {
        Self {
            pace,
            turn: tokio::sync::Mutex::new(None),
            pause: Mutex::new(None),
        }
    }

    /// Sends one market request with `send` when its turn comes, signed in
    /// or out. While Steam's pause lasts, nothing is sent. A 429 pauses the
    /// market; a server error is asked once more, a while later; anything
    /// else ends the pause. No answer, or a server error twice, is
    /// [`Lookup::Unanswered`]; errs when the market said no.
    pub(crate) async fn send<F, Fut>(
        &self,
        signed_in: bool,
        send: F,
    ) -> anyhow::Result<Lookup<Reply>>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = anyhow::Result<Reply>>,
    {
        let mut last = self.turn.lock().await;
        if let Some(pause) = self.paused() {
            return Ok(Lookup::Paused(pause));
        }
        let mut asked_again = false;
        loop {
            if let Some(at) = *last {
                tokio::time::sleep_until(at + self.gap(signed_in)).await;
            }
            let answer = send().await;
            *last = Some(Instant::now());
            let reply = match answer {
                Ok(reply) => reply,
                Err(e) => return Ok(Lookup::Unanswered(e.to_string())),
            };
            if reply.status == StatusCode::TOO_MANY_REQUESTS {
                return Ok(Lookup::Paused(self.turned_down()));
            }
            *self.pause.lock().unwrap() = None;
            if reply.status.is_server_error() && !asked_again {
                asked_again = true;
                tokio::time::sleep(self.pace.after_server_error).await;
                continue;
            }
            if reply.status.is_server_error() {
                return Ok(Lookup::Unanswered(format!(
                    "steamcommunity.com's market said {}, twice",
                    reply.status
                )));
            }
            if !reply.status.is_success() {
                bail!("steamcommunity.com's market said {}", reply.status);
            }
            return Ok(Lookup::Found(reply));
        }
    }

    /// Steam's pause: while it lasts, and once over, until a request gets
    /// through.
    pub(crate) fn pause(&self) -> Option<MarketPause> {
        self.pause.lock().unwrap().map(|(_, pause)| pause)
    }

    /// Takes up a pause from before a restart.
    pub(crate) fn resume(&self, pause: MarketPause) {
        let left = (pause.until - Utc::now()).to_std().unwrap_or_default();
        *self.pause.lock().unwrap() = Some((Instant::now() + left, pause));
    }

    /// The pause, while it lasts: until either clock says it's over. Tokio's
    /// doesn't count the time a computer sleeps, and Steam's pause does.
    fn paused(&self) -> Option<MarketPause> {
        let (until, pause) = (*self.pause.lock().unwrap())?;
        (Instant::now() < until && Utc::now() < pause.until).then_some(pause)
    }

    /// Steam turned a request down: a pause, twice as long as the last if
    /// this was the request sent to see.
    fn turned_down(&self) -> MarketPause {
        let mut paused = self.pause.lock().unwrap();
        let step = paused.map_or(self.pace.first_pause, |(_, last)| {
            (last.step * 2).min(self.pace.longest_pause)
        });
        let pause = MarketPause {
            until: Utc::now() + TimeDelta::from_std(step).unwrap_or_default(),
            step,
        };
        *paused = Some((Instant::now() + step, pause));
        pause
    }

    fn gap(&self, signed_in: bool) -> Duration {
        if signed_in {
            self.pace.signed_in + rand::random_range(Duration::ZERO..=self.pace.jitter)
        } else {
            self.pace.signed_out
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use anyhow::anyhow;

    use super::*;

    const SECOND: Duration = Duration::from_secs(1);
    const MINUTE: Duration = Duration::from_secs(60);

    /// A market that answers with these statuses in turn, and notes when
    /// each request went.
    #[derive(Clone, Default)]
    struct Stand {
        answers: Arc<Mutex<VecDeque<u16>>>,
        sent: Arc<Mutex<Vec<Instant>>>,
    }

    impl Stand {
        fn answering(statuses: &[u16]) -> Self {
            let stand = Self::default();
            stand.answers.lock().unwrap().extend(statuses);
            stand
        }

        async fn reply(&self) -> anyhow::Result<Reply> {
            self.sent.lock().unwrap().push(Instant::now());
            let status = self.answers.lock().unwrap().pop_front().unwrap_or(200);
            Ok(Reply {
                status: StatusCode::from_u16(status).unwrap(),
                html: false,
                body: "{}".into(),
            })
        }

        /// How long after the first each request went.
        fn gaps(&self, start: Instant) -> Vec<Duration> {
            let sent = self.sent.lock().unwrap();
            sent.iter().map(|at| *at - start).collect()
        }
    }

    async fn ask(queue: &MarketQueue, stand: &Stand, signed_in: bool) -> Lookup<StatusCode> {
        match queue.send(signed_in, || stand.reply()).await {
            Ok(Lookup::Found(reply)) => Lookup::Found(reply.status),
            Ok(Lookup::Paused(p)) => Lookup::Paused(p),
            Ok(Lookup::Unanswered(why)) => Lookup::Unanswered(why),
            Err(e) => panic!("{e}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn signed_in_requests_go_five_seconds_apart_and_up_to_a_second_more() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::default();
        let start = Instant::now();

        for _ in 0..4 {
            ask(&queue, &stand, true).await;
        }

        let sent = stand.gaps(start);
        assert_eq!(sent[0], Duration::ZERO, "the first goes at once");
        for pair in sent.windows(2) {
            let gap = pair[1] - pair[0];
            assert!((5 * SECOND..=6 * SECOND).contains(&gap), "{gap:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn signed_out_requests_go_twelve_seconds_apart() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::default();
        let start = Instant::now();

        for _ in 0..3 {
            ask(&queue, &stand, false).await;
        }

        assert_eq!(
            stand.gaps(start),
            [Duration::ZERO, 12 * SECOND, 24 * SECOND]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn requests_go_one_at_a_time() {
        let queue = Arc::new(MarketQueue::new(MarketPace::default()));
        let stand = Stand::default();
        let start = Instant::now();

        let together: Vec<_> = (0..3)
            .map(|_| {
                let (queue, stand) = (Arc::clone(&queue), stand.clone());
                tokio::spawn(async move { ask(&queue, &stand, false).await })
            })
            .collect();
        for one in together {
            one.await.unwrap();
        }

        assert_eq!(
            stand.gaps(start),
            [Duration::ZERO, 12 * SECOND, 24 * SECOND]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_turned_down_pauses_the_market_and_nothing_is_asked_meanwhile() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[429, 429, 429, 429, 429]);
        let start = Instant::now();

        let Lookup::Paused(first) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(first.step, 10 * MINUTE);
        tokio::time::sleep(9 * MINUTE).await;
        assert!(matches!(ask(&queue, &stand, true).await, Lookup::Paused(_)));
        assert_eq!(stand.gaps(start).len(), 1, "nothing asked during the pause");

        // Once it's over, one goes to see; turned down again, it doubles.
        let mut steps = Vec::new();
        for _ in 0..4 {
            let pause = queue.pause().unwrap();
            let left = (pause.until - Utc::now()).to_std().unwrap_or_default();
            tokio::time::sleep(left + SECOND).await;
            let Lookup::Paused(p) = ask(&queue, &stand, true).await else {
                panic!("paused");
            };
            steps.push(p.step);
        }
        assert_eq!(
            steps,
            [20 * MINUTE, 40 * MINUTE, 60 * MINUTE, 60 * MINUTE],
            "20, 40 and 60 minutes, and never longer"
        );
        assert_eq!(stand.gaps(start).len(), 5, "one request each time");
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_that_gets_through_ends_the_pause() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[429]);
        ask(&queue, &stand, true).await;
        tokio::time::sleep(10 * MINUTE).await;

        assert_eq!(
            ask(&queue, &stand, true).await,
            Lookup::Found(StatusCode::OK)
        );
        assert_eq!(queue.pause(), None);

        let stand = Stand::answering(&[429]);
        let Lookup::Paused(p) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(p.step, 10 * MINUTE, "a new pause starts at 10 minutes");
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_that_goes_unanswered_says_so() {
        let queue = MarketQueue::new(MarketPace::default());

        let answer = queue
            .send(true, || async {
                Err(anyhow!("steamcommunity.com didn't answer (timed out)"))
            })
            .await
            .unwrap();

        assert!(
            matches!(&answer, Lookup::Unanswered(why) if why == "steamcommunity.com didn't answer (timed out)"),
            "not an error: nothing wrong with what was asked for"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_is_over_once_the_systems_clock_says_so() {
        // The computer slept through it: tokio's clock didn't move.
        let queue = MarketQueue::new(MarketPace::default());
        *queue.pause.lock().unwrap() = Some((
            Instant::now() + 10 * MINUTE,
            MarketPause {
                until: Utc::now() - TimeDelta::seconds(1),
                step: 10 * MINUTE,
            },
        ));
        let stand = Stand::default();

        assert_eq!(
            ask(&queue, &stand, true).await,
            Lookup::Found(StatusCode::OK)
        );
        assert_eq!(queue.pause(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_from_before_a_restart_holds() {
        let queue = MarketQueue::new(MarketPace::default());
        queue.resume(MarketPause {
            until: Utc::now() + TimeDelta::minutes(40),
            step: 40 * MINUTE,
        });
        let stand = Stand::answering(&[429]);

        assert!(matches!(ask(&queue, &stand, true).await, Lookup::Paused(_)));
        assert!(stand.gaps(Instant::now()).is_empty(), "nothing asked");
        tokio::time::sleep(40 * MINUTE + SECOND).await;
        let Lookup::Paused(p) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(p.step, 60 * MINUTE, "twice 40 minutes, but an hour at most");
    }

    #[tokio::test(start_paused = true)]
    async fn a_server_error_is_asked_once_more_thirty_seconds_later() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[502]);
        let start = Instant::now();

        assert_eq!(
            ask(&queue, &stand, true).await,
            Lookup::Found(StatusCode::OK)
        );
        let sent = stand.gaps(start);
        assert!(sent[1] - sent[0] >= 30 * SECOND, "{sent:?}");

        let stand = Stand::answering(&[500, 503]);
        assert_eq!(
            ask(&queue, &stand, true).await,
            Lookup::Unanswered(
                "steamcommunity.com's market said 503 Service Unavailable, twice".into()
            ),
            "no answer: nothing wrong with what was asked for"
        );
        let stand = Stand::answering(&[404]);
        let e = queue.send(true, || stand.reply()).await.unwrap_err();
        assert!(e.to_string().ends_with("said 404 Not Found"), "{e}");
    }
}
