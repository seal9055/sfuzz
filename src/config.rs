/// Method used to track coverage
pub const COVMETHOD: CovMethod = CovMethod::Block;

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum CovMethod {
    /// Don't track coverage
    None,

    /// Track block level coverage without hit-counters (basically free performance wise)
    Block,

    /// Track block level coverage with hit-counters (30% performance hit)
    BlockHitCounter,

    /// Track edge level coverage without hit-counters
    Edge,

    /// Track edge level coverage with hit-counters
    EdgeHitCounter,
}
