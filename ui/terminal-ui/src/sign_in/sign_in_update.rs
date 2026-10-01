use account::LoginChallenge;

/// How signing in is going: a new code to show, or that it's over, and why
/// not when it didn't work.
pub struct SignInUpdate {
    pub challenge: Option<LoginChallenge>,
    pub done: bool,
    pub err: Option<String>,
}
