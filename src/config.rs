use crate::error_exit;

use std::sync::OnceLock;

use clap::Parser;
use parse_int::parse;

/// Method used to track coverage, currently only Edge and Block coverage is implemented
pub static COV_METHOD: OnceLock<CovMethod> = OnceLock::new();

/// Address at which the fuzzer attempts to create a snapshot once reached
pub static SNAPSHOT_ADDR: OnceLock<Option<usize>> = OnceLock::new();

/// Number of cores to run the fuzzer with
pub static NUM_THREADS: OnceLock<usize> = OnceLock::new();

/// Path to directory to which fuzzer-outputs are saved
pub static OUTPUT_DIR: OnceLock<String> = OnceLock::new();

/// File that contains the user-supplied dictionary
pub static DICT_FILE: OnceLock<Option<String>> = OnceLock::new();

/// Input provided as argument to the target being fuzzed
pub static FUZZ_INPUT: OnceLock<String> = OnceLock::new();

/// Toggle-able permission checks. Should be left on, except for very special cases/debugging
pub static NO_PERM_CHECKS: OnceLock<bool> = OnceLock::new();

/// Additional information is printed out, alongside rolling statistics. Some parts of this only
/// work while running single-threaded
pub static DEBUG_PRINT: OnceLock<bool> = OnceLock::new();

/// In addition to the default printouts, the fuzzer will now also send the data to a remote server.
/// Implemented to interact with api of "https://github.com/rsalz47/cs326-final-gimel"
pub static SEND_REMOTE: OnceLock<Option<String>> = OnceLock::new();

/// Separates branch-if-equal comparisons into multiple separate compares that benefit from
/// coverage tracking so larger magic numbers can still be found through fuzzing
pub static CMP_COV: OnceLock<bool> = OnceLock::new();

/// Manually override the automatically calibrated timeout
pub static OVERRIDE_TIMEOUT: OnceLock<Option<u64>> = OnceLock::new();

/// Collect a full register trace of program execution, for large programs, it can take several
/// hours to write out a single case, only enable when debugging the JIT. Only works when fuzzer is
/// being run single-threaded
pub static FULL_TRACE: OnceLock<bool> = OnceLock::new();

/// Amount of cases that will be run before the fuzzer automatically shuts down
pub static RUN_CASES: OnceLock<Option<usize>> = OnceLock::new();

/// Size of memory space allocated for each thread's virtual address space
pub const MAX_GUEST_ADDR: usize = 64 * 1024 * 1024;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum CovMethod {
    /// Don't track coverage
    None,

    /// Track block level coverage 
    Block,

    /// Track edge level coverage
    Edge,

    /// Track edge level coverage alongside a call-stack hash to get more fine-grained coverage
    /// results
    CallStack,
}

/// Used by clap to parse command-line arguments
#[derive(Debug, Parser)]
#[clap(author = "seal9055", version, about = "Coverage-guided emulation based fuzzer")]
#[clap(override_usage = "sfuzz [OPTION] -- /path/to/fuzzed_app [ ... ] (use `@@` to specify \
    position of fuzz-input in target-argv)\n\n    ex: sfuzz -i in -o out -n 16 -- \
    ./test_cases/test @@")]
pub struct Cli {
    #[clap(short, value_name = "DIR", forbid_empty_values = true, display_order = 1)]
    /// - Input directory that should contain the initial seed files
    pub input_dir: String,

    #[clap(short, value_name = "DIR", forbid_empty_values = true, display_order = 2)]
    /// - Output directory that will be used to eg. save crashes
    pub output_dir: String,

    #[clap(short = 'V', takes_value = false)]
    /// - Print version information
    pub version: bool,

    #[clap(short = 'h', takes_value = false)]
    /// - Print help information
    pub help: bool,

    #[clap(default_value_t=1, short, help_heading = "CONFIG")]
    /// - The number of threads to run this fuzzer with
    pub num_threads: usize,

    #[clap(short = 'p', help_heading = "CONFIG", takes_value = false)]
    /// - Disables permission checking, highly discouraged since it will cause the fuzzer itself to
    /// segfault when the target crashes due to being run in an emulator
    pub no_perm_checks: bool,

    #[clap(short = 'C', help_heading = "CONFIG", takes_value = false)]
    /// - Disables CmpCov, results in slight performance increase, but makes it almost impossible
    /// for the fuzzer to get past large magic value comparisons
    pub no_cmp_cov: bool,

    #[clap(short = 'e', help_heading = "CONFIG")]
    /// - File extension for the fuzz test input file if the target requires it
    pub extension: Option<String>,

    #[clap(short = 'D', help_heading = "CONFIG", takes_value = false)]
    /// - Enable a rolling debug-print and information on which functions are lifted instead of the
    /// default print-window
    pub debug_print: bool,

    #[clap(short = 'k', help_heading = "CONFIG", takes_value = true)]
    /// - In addition to displaying the data on the screen, also send it to a remote api. Provide 
    /// port and ip to send it to in format "127.0.0.1:9055". Implemented to interact with api of
    /// https://github.com/rsalz47/cs326-final-gimel
    pub send_remote: Option<String>,

    #[clap(short = 's', help_heading = "CONFIG")]
    /// - Take a snapshot of the target at specified address and launch future fuzz-cases off of this
    /// snapshot
    pub snapshot: Option<String>,

    #[clap(short = 't', help_heading = "CONFIG")]
    /// - Override the timeout that is otherwise dynamically set during calibration phase
    pub override_timeout: Option<u64>,

    #[clap(short = 'r', help_heading = "CONFIG")]
    /// - Optionally set the amount of cases to be run before the fuzzer shuts down
    pub run_cases: Option<String>,

    #[clap(short = 'f', help_heading = "CONFIG", takes_value = false)]
    /// - Collect a full register trace of program execution. Only enable while debugging, majorly
    /// slows down performance. Only works when fuzzer is run single-threaded
    pub full_trace: bool,

    #[clap(short = 'd', value_name = "DICT", help_heading = "CONFIG", forbid_empty_values = true)]
    /// - Optionally supply a new-line separated list of inputs that will be mutated into the 
    /// fuzz-inputs
    pub dictionary: Option<String>,

    #[clap(short = 'c', help_heading = "CONFIG", default_value = "edge")]
    /// - Coverage method, currently supports `edge`, `block`, and `call-stack` based coverage
    pub cov_method: String,

    #[clap(last = true)]
    /// The target to be fuzzed alongside its arguments
    pub fuzzed_app: Vec<String>,
}

/// Initialize configuration variables based on passed in commandline arguments, and verify that
/// the user properly setup their fuzz-case
pub fn handle_cli(args: &mut Cli) {
    NUM_THREADS.set(args.num_threads).unwrap();
    NO_PERM_CHECKS.set(args.no_perm_checks).unwrap();
    DEBUG_PRINT.set(args.debug_print).unwrap();
    SEND_REMOTE.set(args.send_remote.clone()).unwrap();
    OVERRIDE_TIMEOUT.set(args.override_timeout).unwrap();
    CMP_COV.set(!args.no_cmp_cov).unwrap();

    if args.fuzzed_app.is_empty() {
        error_exit("You need to specify the target to be fuzzed");
    }

    // Verify that the input and output directories are valid
    if !std::path::Path::new(&args.input_dir).is_dir() {
        error_exit("You need to specify a valid input directory");
    }
    if !std::path::Path::new(&args.output_dir).is_dir() {
        error_exit("You need to specify a valid output directory");
    }
    OUTPUT_DIR.set(args.output_dir.clone()).unwrap();

    if let Some(dict) = &args.dictionary {
        if !std::path::Path::new(&dict).is_file() {
            error_exit("You need to specify a valid dictionary file");
        }
        DICT_FILE.set(Some(dict.to_string())).unwrap();
    } else {
        DICT_FILE.set(None).unwrap();
    }

    // Create the directory to save crashes too
    let mut crash_dir = args.output_dir.clone();
    crash_dir.push_str("/crashes");
    std::fs::create_dir_all(crash_dir).unwrap();

    // Set the fuzz-input. If the user specified an extension, add that too
    FUZZ_INPUT.set(
        if let Some(ext) = &args.extension {
            format!("fuzz_input.{}\0", ext)
        } else {
            "fuzz_input\0".to_string()
        }
    ).unwrap();

    // Verify that the user supplied `@@` and use it to setup the fuzz-input's argv
    let index = args.fuzzed_app.iter().position(|e| e == "@@").unwrap_or_else(|| {
        error_exit("You need to specify how the fuzz-case input files should be passed in. This \
                   can be done using the `@@` flag as shown in the example under `Usage`.");
    });
    args.fuzzed_app[index] = FUZZ_INPUT.get().unwrap().to_string();

    // Set snapshot address if requested
    if let Some(ss) = &args.snapshot {
        let num_repr = parse::<usize>(&ss).unwrap();
        SNAPSHOT_ADDR.set(Some(num_repr)).unwrap();
    } else {
        SNAPSHOT_ADDR.set(None).unwrap();
    }

    // Set max number of cases if requested
    if let Some(runs) = &args.run_cases {
        let num_repr = parse::<usize>(&runs).unwrap();
        RUN_CASES.set(Some(num_repr)).unwrap();
    } else {
        RUN_CASES.set(None).unwrap();
    }

    // Set the coverage collection method
    match args.cov_method.as_str() {
        "edge" => {
            COV_METHOD.set(CovMethod::Edge).unwrap();
        },
        "block" => {
            COV_METHOD.set(CovMethod::Block).unwrap();
        },
        "call-stack" => {
            COV_METHOD.set(CovMethod::CallStack).unwrap();
        },
        _ => {
            error_exit("You're specified coverage method is not supported, please chose `edge`, \
                       `block`, or `call-stack`")
        },
    }

    // Trace mode
    if args.full_trace == true && args.num_threads != 1 {
        error_exit("Full Trace mode only works when running single-threaded");
    } else {
        FULL_TRACE.set(args.full_trace).unwrap();
    }

    if false {
        println!("cov_method: {:?}", COV_METHOD);
        println!("snapshot_addr: {:?}", SNAPSHOT_ADDR);
        println!("num_threads: {:?}", NUM_THREADS);
        println!("output_dir: {:?}", OUTPUT_DIR);
        println!("fuzz_input: {:?}", FUZZ_INPUT);
        println!("no_perm_checks: {:?}", NO_PERM_CHECKS);
        println!("debug_print: {:?}", DEBUG_PRINT);
        println!("send_remote: {:?}", SEND_REMOTE);
        println!("override_timeout: {:?}", OVERRIDE_TIMEOUT);
        println!("full_trace: {:?}", FULL_TRACE);
    }
}

