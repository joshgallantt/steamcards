use crate::dto::BadgeDto;

/// One page of badges, as read.
pub(crate) struct BadgePageDto {
    /// The games with trading cards on this page.
    pub(crate) games: Vec<BadgeDto>,
    /// How many pages of badges there are.
    pub(crate) pages: u32,
}
