/// The signed-in Steam account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Its sign-in name; empty when Steam didn't say.
    pub name: String,
    /// A sign-in is saved but Steam no longer takes it: signing in again is
    /// the way on.
    pub expired: bool,
}
