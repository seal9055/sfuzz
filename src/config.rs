
/// Method used to track coverage, currently only Edge and Block coverage is implemented
pub const COVMETHOD: CovMethod = CovMethod::Edge;

/// Address at which the fuzzer attempts to create a snapshot once reached
pub const SNAPSHOT_ADDR: Option<usize> = None;

/// Number of cores to run the fuzzer with
pub const NUM_THREADS: usize = 16;

/// Count number of instructions executed by test cases
pub const COUNT_INSTRS: bool = true;

/// Toggle-able permission checks
pub const PERM_CHECKS: bool = true;

/// Additional information is printed out, alongside rolling statistics. Some parts of this only
/// work while running single-threaded
pub const DEBUG_PRINT: bool = true;

/// Manually override the automatically calibrated timeout
pub const OVERRIDE_TIMEOUT: Option<u64> = None;

/// Collect a full register trace of program execution, for large programs, it can take several
/// hours to write out a single case, only enable when debugging the JIT. Only works when fuzzer is
/// being run single-threaded
pub const FULL_TRACE: bool = false;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum CovMethod {
    /// Don't track coverage
    None,

    /// Track block level coverage without hit-counters (basically free performance wise)
    Block,

    /// Track edge level coverage without hit-counters
    Edge,
}
