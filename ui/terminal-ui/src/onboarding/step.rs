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
