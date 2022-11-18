#![feature(once_cell)]

use sfuzz::{
    mmu::Perms,
    emulator::{Emulator, Register, Fault, ExitType},
    jit::{Jit, LibFuncs},
    pretty_printing::{print_stats, log, LogType, emit_trace},
    Input, Corpus, Statistics, error_exit, load_elf_segments, worker, snapshot, calibrate_seeds,
    config::{handle_cli, Cli, SNAPSHOT_ADDR, OVERRIDE_TIMEOUT, NUM_THREADS, MAX_GUEST_ADDR, 
        RUN_CASES},
};
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use byteorder::{LittleEndian, WriteBytesExt};
use rustc_hash::FxHashMap;
use console::Term;
use clap::Parser;

/// Hook that makes use of sfuzz's mmu to perform a memory safe malloc operation
fn malloc_hook(emu: &mut Emulator) -> Result<(), Fault> {
    let alloc_size = emu.get_reg(Register::A1);

    if let Some(addr) = emu.memory.allocate(alloc_size, Perms::READ | Perms::WRITE) {
        emu.set_reg(Register::A0, addr);
        emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
        Ok(())
    } else {
        Err(Fault::OOM)
    }
}

/// Hook that makes use of sfuzz's mmu to perform a memory safe calloc operation, pretty much same
/// as malloc apart from how the size is calculated
fn calloc_hook(emu: &mut Emulator) -> Result<(), Fault> {
    let nmemb = emu.get_reg(Register::A1);
    let size  = emu.get_reg(Register::A2);
    let alloc_size = size * nmemb;

    if let Some(addr) = emu.memory.allocate(alloc_size, Perms::READ | Perms::WRITE) {
        emu.set_reg(Register::A0, addr);
        emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
        Ok(())
    } else {
        Err(Fault::OOM)
    }
}

/// Hook that makes use of sfuzz's mmu to perform a memory safe free operation
fn free_hook(emu: &mut Emulator) -> Result<(), Fault> {
    let ptr = emu.get_reg(Register::A1);

    emu.memory.free(ptr)?;
    emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
    Ok(())
}

/// Inserts various hooks into binary
fn insert_hooks(sym_map: &FxHashMap<String, usize>, emu: &mut Emulator) {
    match sym_map.get("_free_r") {
        Some(v) => {
            log(LogType::Success, "_free_r hooked");
            emu.hooks.insert(*v, free_hook);
        },
        None => {
            log(LogType::Neutral, "free_r does not exist in target so it could not be hooked"); 
        }
    }

    match sym_map.get("_malloc_r") {
        Some(v) => {
            log(LogType::Success, "_malloc_r hooked");
            emu.hooks.insert(*v, malloc_hook);
        },
        None => {
            log(LogType::Neutral, "malloc_r does not exist in target so it could not be hooked"); 
        }
    }

    match sym_map.get("_calloc_r") {
        Some(v) => {
            log(LogType::Success, "_calloc_r hooked");
            emu.hooks.insert(*v, calloc_hook);
        },
        None => {
            log(LogType::Neutral, "_calloc_r does not exist in target so it could not be hooked"); 
        }
    }

    // Hooks for strlen and strcmp are required because the default libc variants go out of bounds.
    // This is not a security issue since the functions verify that everything is properly aligned,
    // but since this fuzzer notices byte level permission violations these are required.

    match sym_map.get("strlen") {
        Some(v) => {
            log(LogType::Success, "strlen replaced with safe implementation");
            emu.custom_lib.insert(*v, LibFuncs::STRLEN);
        },
        None => {
            log(LogType::Neutral, "strlen does not exist in target so it could not be hooked"); 
        }
    }

    match sym_map.get("strcmp") {
        Some(v) => {
            log(LogType::Success, "strcmp replaced with safe implementation");
            emu.custom_lib.insert(*v, LibFuncs::STRCMP);
        },
        None => { 
            log(LogType::Neutral, "strcmp does not exist in target so it could not be hooked"); 
        }
    }
}

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() -> std::io::Result<()> {
    // Thead-shared jit backing
    let jit = Arc::new(Jit::new(16 * 1024 * 1024));

    // Thread-shared mutex that is used to lock JIT-compilation
    let prevent_rc: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    // Thread-shared structure that holds fuzz-inputs and coverage information
    let mut corpus: Corpus = Corpus::new(16*1024*1024);

    // Each thread gets its own forked emulator. The jit-cache is shared between them however
    let mut emu = Emulator::new(MAX_GUEST_ADDR, jit, prevent_rc);

    // Statistics structure. This is kept local to the main thread and updated via message passing 
    // from the worker threads
    let mut stats = Statistics::default();

    // Messaging objects used to transfer statistics between worker threads and main thread
    let (tx, rx): (Sender<Statistics>, Receiver<Statistics>) = mpsc::channel();

    let term = Term::buffered_stdout();
    term.clear_screen()?;

    // Parse commandline-args and set config variables based on them
    let mut args = Cli::parse();
    handle_cli(&mut args);

    // Insert loadable segments into emulator address space and retrieve symbol table information
    let sym_map = load_elf_segments(&args.fuzzed_app[0], &mut emu).unwrap_or_else(||{
        error_exit("Unrecoverable error while loading elf segments");
    });

    // Initialize corpus with files from input directory
    let mut w = corpus.inputs.write();
    for filename in std::fs::read_dir(args.input_dir)? {
        let filename = filename?.path();
        let data = std::fs::read(filename)?;

        // Add the corpus input to the corpus
        w.push(Input::new(data, None));
    }
    if w.is_empty() { panic!("Please supply at least 1 initial seed"); }
    drop(w);

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE)
        .expect("Error allocating stack");
    emu.set_reg(Register::Sp, (stack + (1024 * 1024)) - 8);

    // Setup arguments
    //let arguments = vec!["test_cases/harder_test\0".to_string(), "fuzz_input\0".to_string()];
    let argv: Vec<usize> = args.fuzzed_app.iter().map(|e| {
        let addr = emu.allocate(64, Perms::READ | Perms::WRITE)
            .expect("Allocating an argument failed");
        emu.memory.write_mem(addr, e.as_bytes(), e.len()).expect("Writing to argv[0] failed");
        addr
    }).collect();

    // Macro to push 64-bit integers onto the stack
    macro_rules! push {
        ($expr:expr) => {
            let sp = emu.get_reg(Register::Sp) - 8;
            let mut wtr = vec![];
            wtr.write_u64::<LittleEndian>($expr as u64)?;
            emu.memory.write_mem(sp, &wtr, 8).unwrap();
            emu.set_reg(Register::Sp, sp);
        }
    }

    // Setup argc, argv & envp
    push!(0u64);            // Auxp
    push!(0u64);            // Envp
    push!(0u64);            // Null-terminate Argv
    for arg in argv.iter().rev() {
        push!(*arg);
    }
    push!(argv.len());    // Argc

    // Insert various hooks into binary
    insert_hooks(&sym_map, &mut emu);

    // Setup snapshot fuzzing at a point before the fuzz-input is read in
    if let Some(addr) = SNAPSHOT_ADDR.get().unwrap() {
        println!("Activated snapshot-based fuzzing");

        // Insert snapshot fuzzer exit condition
        emu.exit_conds.insert(*addr, ExitType::Snapshot);

        // Snapshot the emulator
        snapshot(&mut emu, &corpus);
    }

    // Calibrate the emulator for the timeout.
    // Alternatively configs can be used to override automatically determined timeout
    let mut first_hit_cov;
    (emu.timeout, first_hit_cov) = calibrate_seeds(&mut emu, &corpus);
    if let Some(v) = OVERRIDE_TIMEOUT.get().unwrap() {
        emu.timeout = *v;
    }

    // Reset coverage collected during initial callibration so it is in a default state once
    // fuzzing actually starts. This also removes the coverage generated while capturing the
    // initial snapshot
    corpus.reset_coverage();

    let emu = Arc::new(emu);
    let corpus = Arc::new(corpus);

    // Spawn worker threads to do the actual fuzzing
    for thr_id in 0..*NUM_THREADS.get().unwrap() {
        let emu_cp = emu.fork();
        let corpus = corpus.clone();
        let tx = tx.clone();

        thread::spawn(move || worker(thr_id, emu_cp, corpus, tx));
    }

    // Continuous statistic tracking via message passing in main thread
    let start = Instant::now();
    let mut last_time = Instant::now();
    let mut last_cov_event: f64 = 0.0;

    // Sleep for short duration on startup before printing statistics, otherwise elapsed time might
    // be 0, leading to a crash while printing statistics
    thread::sleep(Duration::from_millis(1000));

    // Update stats structure whenever a thread sends a new message
    for received in rx {
        let elapsed_time = start.elapsed().as_secs_f64();


        // Check if we got new coverage
        if received.coverage != 0 || received.cmpcov != 0 {
            last_cov_event = elapsed_time;
        }

        if true {
            first_hit_cov.extend(&(received.first_hit_cov.unwrap()));
            if !first_hit_cov.is_empty() {
                emit_trace(&first_hit_cov);
                first_hit_cov.clear();
            }
        }

        stats.coverage    += received.coverage;
        stats.cmpcov      += received.cmpcov;
        stats.total_cases += received.total_cases;
        stats.crashes     += received.crashes;
        stats.ucrashes    += received.ucrashes;
        stats.instr_count += received.instr_count;
        stats.timeouts    += received.timeouts;

        // Print out updated statistics every second
        if last_time.elapsed() >= Duration::from_millis(500) {
            print_stats(&term, &stats, elapsed_time, emu.timeout, &corpus, last_cov_event);
            last_time = Instant::now();
        }

        if let Some(max_cases) = RUN_CASES.get().unwrap() {
            if stats.total_cases >= *max_cases {
                error_exit("Fuzzer reached specified maximum number of total cases");

            }
        }
    }

    Ok(())
}
