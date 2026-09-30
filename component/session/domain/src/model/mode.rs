/// How the games being played are farmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// One game on its own, until its cards have dropped.
    Cards,
    /// Several together, building up hours until their cards can drop.
    Hours,
}
