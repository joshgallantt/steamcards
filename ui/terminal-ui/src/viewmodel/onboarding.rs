use std::time::Duration;

use account::GetAccount;
use farming::farm_order;
use market::Basis;

use super::screen::Snapshot;

/// The steps of getting set up, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    /// Sign in with the Steam app. Can't be skipped.
    SignIn,
    /// Pick priority games from the ones with cards left. Can be skipped.
    Games,
    Start,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Welcome, Step::SignIn, Step::Games, Step::Start];
}

/// Nothing can be farmed without an account, so onboarding won't get past
/// signing in until the account is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedsAccount;

/// First run: a welcome, signing in, picking games, starting to farm. The app
/// comes back to signing in when the account signs out.
pub struct Onboarding {
    account: GetAccount,
    /// `None` once onboarding is over.
    step: Option<Step>,
}

impl Onboarding {
    /// Starts at the welcome when nobody is signed in; otherwise there is
    /// nothing to set up.
    pub fn new(account: GetAccount) -> Self {
        let step = account().is_none().then_some(Step::Welcome);
        Self { account, step }
    }

    /// The step on screen, or `None` once onboarding is over.
    pub fn step(&self) -> Option<Step> {
        self.step
    }

    pub fn is_active(&self) -> bool {
        self.step.is_some()
    }

    /// Whether the sign-in step would let the user on.
    pub fn can_continue(&self) -> bool {
        self.signed_in()
    }

    /// Moves on a step; after the last one, onboarding is over and farming
    /// can start. Getting past signing in — or starting — takes an account.
    pub fn forward(&mut self) -> Result<(), NeedsAccount> {
        let Some(step) = self.step else {
            return Ok(());
        };
        if matches!(step, Step::SignIn | Step::Start) && !self.signed_in() {
            return Err(NeedsAccount);
        }
        self.step = match step {
            Step::Welcome => Some(Step::SignIn),
            Step::SignIn => Some(Step::Games),
            Step::Games => Some(Step::Start),
            Step::Start => None,
        };
        Ok(())
    }

    /// Goes back a step; the welcome is as far back as it goes.
    pub fn back(&mut self) {
        self.step = self.step.map(|s| match s {
            Step::Welcome | Step::SignIn => Step::Welcome,
            Step::Games => Step::SignIn,
            Step::Start => Step::Games,
        });
    }

    /// After signing out: back to signing in.
    pub fn signed_out(&mut self) {
        self.step = Some(Step::SignIn);
    }

    fn signed_in(&self) -> bool {
        (self.account)().is_some()
    }
}

/// What the Start step sizes up before farming begins (docs/design/ui.md,
/// mockup o): who's signed in, the games picked, how friends see the games,
/// the games and drops to farm in the order the farmer will take them, how
/// long that should take at first, and how values are read.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    /// The account's name, when signed in: empty when Steam didn't say it.
    pub account: Option<String>,
    /// The priority games, by name, in rank order.
    pub picked: Vec<String>,
    pub only_priority: bool,
    pub appear_online: bool,
    /// The games to farm and their drops; `None` until the library is read.
    pub to_farm: Option<(usize, u32)>,
    /// The time to finish, and the part of it building hours, as first
    /// worked out: at 30 minutes a drop, until drops teach it better.
    pub eta: Option<(Duration, Duration)>,
    pub basis: Basis,
}

impl Job {
    pub fn build(s: &Snapshot<'_>) -> Self {
        let library = s.library();
        let set_aside = s.status.map_or(&[][..], |st| st.set_aside.as_slice());
        let order = farm_order(library, s.prefs, set_aside);
        let drops = order
            .iter()
            .filter_map(|&id| library.game(id))
            .map(|g| g.drops.remaining)
            .sum();
        Self {
            account: s.account.map(|a| a.name.clone()),
            picked: s
                .prefs
                .priority_games
                .iter()
                .map(|&id| s.name(id))
                .collect(),
            only_priority: s.prefs.only_priority,
            appear_online: s.prefs.appear_online,
            to_farm: (!library.is_empty()).then_some((order.len(), drops)),
            eta: s.forecast.map(|f| (f.eta, f.hours_term)),
            basis: s.basis,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use account::Account;

    use super::*;

    /// The account, signed in or not as the flag says.
    fn steam(signed_in: &Arc<AtomicBool>) -> GetAccount {
        let signed_in = signed_in.clone();
        Arc::new(move || {
            signed_in.load(Ordering::Relaxed).then(|| Account {
                name: "cardfarmer".into(),
                expired: false,
            })
        })
    }

    #[test]
    fn a_first_run_starts_at_the_welcome_and_a_set_up_one_skips_it() {
        let first = Onboarding::new(steam(&Arc::new(AtomicBool::new(false))));
        assert_eq!(first.step(), Some(Step::Welcome));

        let set_up = Onboarding::new(steam(&Arc::new(AtomicBool::new(true))));
        assert!(!set_up.is_active());
    }

    #[test]
    fn getting_past_signing_in_takes_an_account() {
        let signed_in = Arc::new(AtomicBool::new(false));
        let mut o = Onboarding::new(steam(&signed_in));
        assert_eq!(o.forward(), Ok(()));
        assert_eq!(o.step(), Some(Step::SignIn));

        assert!(!o.can_continue());
        assert_eq!(o.forward(), Err(NeedsAccount));
        assert_eq!(o.step(), Some(Step::SignIn), "it stays put");

        signed_in.store(true, Ordering::Relaxed);
        assert_eq!(o.forward(), Ok(()));
        assert_eq!(o.step(), Some(Step::Games));
    }

    #[test]
    fn games_can_be_skipped_and_the_last_step_ends_onboarding() {
        let signed_in = Arc::new(AtomicBool::new(false));
        let mut o = Onboarding::new(steam(&signed_in));
        o.forward().unwrap();
        signed_in.store(true, Ordering::Relaxed);
        o.forward().unwrap();
        o.forward().unwrap();
        assert_eq!(o.step(), Some(Step::Start), "nothing to pick first");
        o.forward().unwrap();
        assert!(!o.is_active());
    }

    #[test]
    fn back_retraces_the_steps() {
        let signed_in = Arc::new(AtomicBool::new(false));
        let mut o = Onboarding::new(steam(&signed_in));
        o.back();
        assert_eq!(o.step(), Some(Step::Welcome), "nowhere further back");
        o.forward().unwrap();
        signed_in.store(true, Ordering::Relaxed);
        for _ in 0..2 {
            o.forward().unwrap();
        }
        assert_eq!(o.step(), Some(Step::Start));
        for back_to in [Step::Games, Step::SignIn, Step::Welcome] {
            o.back();
            assert_eq!(o.step(), Some(back_to));
        }
    }

    #[test]
    fn signing_out_goes_back_to_signing_in() {
        let signed_in = Arc::new(AtomicBool::new(true));
        let mut o = Onboarding::new(steam(&signed_in));
        signed_in.store(false, Ordering::Relaxed);
        o.signed_out();
        assert_eq!(o.step(), Some(Step::SignIn), "not the welcome again");
    }

    #[test]
    fn the_start_sizes_up_the_job_as_the_farmer_will_take_it() {
        let data = crate::viewmodel::fixtures::first_minutes();
        let job = Job::build(&data.snapshot());
        assert_eq!(job.account.as_deref(), Some("alice"));
        assert_eq!(
            job.to_farm,
            Some((62, 252)),
            "62 games · 252 drops: Counter-Strike 2 is skipped"
        );
        let (eta, hours) = job.eta.unwrap();
        assert_eq!(crate::tui::format::eta(eta), "≈ 5d 9h");
        assert_eq!(crate::tui::format::estimate(hours), "3h");
        assert!(job.picked.is_empty() && !job.only_priority && !job.appear_online);
        assert_eq!(job.basis, Basis::List);
    }

    #[test]
    fn only_priority_farms_the_games_picked() {
        let mut data = crate::viewmodel::fixtures::first_minutes();
        data.prefs.priority_games = vec![
            crate::viewmodel::fixtures::STRAY,
            crate::viewmodel::fixtures::LIMBO,
        ];
        data.prefs.only_priority = true;
        let job = Job::build(&data.snapshot());
        assert_eq!(job.picked, ["Stray", "LIMBO"]);
        assert_eq!(job.to_farm, Some((2, 3)), "Stray's 1 and LIMBO's 2");
        let mut unread = crate::viewmodel::fixtures::first_minutes();
        unread.status.library = library::SteamLibrary::default();
        assert_eq!(Job::build(&unread.snapshot()).to_farm, None);
    }
}
