
/// Method used to track coverage, currently only Edge coverage is implemented
pub const COVMETHOD: CovMethod = CovMethod::Edge;

/// Address at which the fuzzer attempts to create a snapshot once reached
pub const SNAPSHOT_ADDR: Option<usize> = None;

/// Number of cores to run the fuzzer with
pub const NUM_THREADS: usize = 1;

/// Count number of instructions executed by test cases
pub const COUNT_INSTRS: bool = true;

/// Toggle-able permission checks
pub const PERM_CHECKS: bool = true;

/// Additional information is printed out, alongside rolling statistics
pub const DEBUG_PRINT: bool = false;

/// Manually override the automatically calibrated timeout
pub const OVERRIDE_TIMEOUT: Option<u64> = Some(10000000);

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
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
