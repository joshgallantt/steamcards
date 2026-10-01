/// What went wrong while farming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// The badges couldn't be read, for this reason.
    BadgesUnread(String),
    /// The connection to Steam went, for this reason.
    Lost(String),
    /// Steam signed this session off while another device played.
    SignedOff,
    /// Another session signed on in this one's place: farming stopped
    /// rather than knock it off.
    Replaced,
}
