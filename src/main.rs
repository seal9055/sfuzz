use sfuzz::{
    mmu::Perms,
    emulator::{Emulator, Register, Fault},
    error_exit, load_elf_segments, worker,
    jit::{Jit, LibFuncs},
    Input, Corpus, Statistics
};
use std::thread;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::atomic::Ordering;

use byteorder::{LittleEndian, WriteBytesExt};
use num_format::{Locale, ToFormattedString};
use rustc_hash::FxHashMap;

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

    emu.memory.free(ptr).unwrap();
    emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
    Ok(())
}

/// Inserts various hooks into binary
fn insert_hooks(sym_map: &FxHashMap<String, usize>, emu: &mut Emulator) {

    // _free_r hook
    match sym_map.get("_free_r") {
        Some(v) => {
            println!("_free_r() hooked");
            emu.hooks.insert(*v, free_hook);
        },
        None => { println!("_free_r does not exist in target so it could not be hooked"); }
    }

    // _malloc_r hook
    match sym_map.get("_malloc_r") {
        Some(v) => {
            println!("_malloc_r() hooked");
            emu.hooks.insert(*v, malloc_hook);
        },
        None => { println!("_malloc_r does not exist in target so it could not be hooked"); }
    }

    // _calloc_r hook
    match sym_map.get("_calloc_r") {
        Some(v) => {
            println!("_calloc_r() hooked");
            emu.hooks.insert(*v, calloc_hook);
        },
        None => { println!("_calloc_r does not exist in target so it could not be hooked"); }
    }

    // Hooks for strlen and strcmp are required because the default libc variants go out of bounds.
    // This is not a security issue since the functions verify that everything is properly aligned,
    // but since this fuzzer notices byte level permission violations these are required.

    // strlen hook
    match sym_map.get("strlen") {
        Some(v) => {
            println!("strlen() replaced with safe implementation");
            emu.custom_lib.insert(*v, LibFuncs::STRLEN);
        },
        None => { println!("strlen() does not exist in target so it could not be hooked"); }
    }

    // strcmp hook
    match sym_map.get("strcmp") {
        Some(v) => {
            println!("strcmp() replaced with safe implementation");
            emu.custom_lib.insert(*v, LibFuncs::STRCMP);
        },
        None => { println!("strcmp() does not exist in target so it could not be hooked"); }
    }
}

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    // Thead-shared jit backing
    let jit = Arc::new(Jit::new(16 * 1024 * 1024));

    // Thread-shared structure that holds fuzz-inputs and coverage information
    let corpus: Corpus = Corpus::new(16*1024*1024);

    // Each thread gets its own forked emulator. The jit-cache is shared between them however
    let mut emu = Emulator::new(64 * 1024 * 1024, jit);

    // Statistics structure. This is kept local to the main thread and updated via message passing 
    // from the worker threads
    let mut stats = Statistics::default();

    // Insert loadable segments into emulator address space and retrieve symbol table information
    let sym_map = load_elf_segments("./test2", &mut emu).unwrap_or_else(||{
        error_exit("Unrecoverable error while loading elf segments");
    });

    // Initialize corpus with every file in ./files
    let mut w = corpus.inputs.write();
    for filename in std::fs::read_dir("files").unwrap() {
        let filename = filename.unwrap().path();
        let data = std::fs::read(filename).unwrap();

        // Add the corpus input to the corpus
        w.push(Input::new(data));
    }
    drop(w);

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE)
        .expect("Error allocating stack");
    emu.set_reg(Register::Sp, (stack + (1024 * 1024)) - 8);

    // Allocate space for argv[0] & argv[1]
    let argv0 = emu.allocate(64, Perms::READ | Perms::WRITE).expect("Allocating argv[0] failed");
    let argv1 = emu.allocate(64, Perms::READ | Perms::WRITE).expect("Allocating argv[1] failed");
    emu.memory.write_mem(argv0, b"test2\0", 6).expect("Writing to argv[0] failed");
    emu.memory.write_mem(argv1, b"fuzz_input\0", 11).expect("Writing to argv[1] failed");

    // Macro to push 64-bit integers onto the stack
    macro_rules! push {
        ($expr:expr) => {
            let sp = emu.get_reg(Register::Sp) - 8;
            let mut wtr = vec![];
            wtr.write_u64::<LittleEndian>($expr as u64).unwrap();
            emu.memory.write_mem(sp, &wtr, 8).unwrap();
            emu.set_reg(Register::Sp, sp);
        }
    }

    // Setup argc, argv & envp
    push!(0u64);    // Auxp
    push!(0u64);    // Envp
    push!(0u64);    // Null-terminate Argv
    push!(argv1);   // Argv[1] 
    push!(argv0);   // Argv[0]
    push!(1u64);    // Argc

    // Insert various hooks into binary
    insert_hooks(&sym_map, &mut emu);
    
    let corpus = Arc::new(corpus);
    let emu = Arc::new(emu);
    let (tx, rx): (Sender<Statistics>, Receiver<Statistics>) = mpsc::channel();

    // Spawn worker threads to do the actual fuzzing
    for thr_id in 0..16 {
        let emu_cp = emu.fork();
        let corpus = corpus.clone();
        let tx = tx.clone();

        thread::spawn(move || worker(thr_id, emu_cp, corpus, tx));
    }

    // Continuous statistic tracking via message passing in main thread
    let start = Instant::now();
    let mut last_time = Instant::now();

    // Update stats structure whenever a thread sends a new message
    for received in rx {
        let elapsed_time = start.elapsed().as_secs_f64();
        stats.total_cases += received.total_cases;
        stats.crashes += received.crashes;
        stats.coverage = corpus.cov_counter.load(Ordering::SeqCst);

        // Print out updated statistics every second
        if last_time.elapsed() >= Duration::from_millis(1000) {
            println!("[{:8.2}] fuzz cases: {:12} : fcps: {:8} : coverage: {:6} : crashes: {:8}", 
                     elapsed_time, 
                     stats.total_cases.to_formatted_string(&Locale::en),
                     (stats.total_cases / elapsed_time as usize).to_formatted_string(&Locale::en), 
                     stats.coverage,
                     stats.crashes);

            //let v = corpus.coverage.lock().unwrap();
            //for a in v.iter() {
            //    println!("{:x?}", a);
            //}

            last_time = Instant::now();
        }
    }
}
