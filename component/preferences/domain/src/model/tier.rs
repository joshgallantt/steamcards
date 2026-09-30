/// How much the user wants a game farmed: priority -> indifferent -> skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// 1-based position among the priority games.
    Priority(usize),
    /// The default: farmed after the priorities, in the farmer's own order.
    Indifferent,
    /// Never farmed.
    Skip,
}
