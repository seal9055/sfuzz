use sfuzz::{
    mmu::Perms,
    emulator::{Emulator, Register, Fault},
    error_exit, load_elf_segments, worker,
    jit::{Jit},
};
use std::thread;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Hook that makes use of sfuzz's mmu to perform a memory safe free operation
fn free_hook(emu: &mut Emulator) -> Result<(), Fault> {
    let ptr = emu.get_reg(Register::A1);

    emu.memory.free(ptr).unwrap();
    emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
    Ok(())
}

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    // Thead-shared jit backing
    let jit = Arc::new(Jit::new(16 * 1024 * 1024));
    let mut emu = Emulator::new(64 * 1024 * 1024, jit);

    // Insert loadable segments into emulator address space and retrieve symbol table information
    let sym_map = load_elf_segments("./test_bin", &mut emu).unwrap_or_else(||{
        error_exit("Unrecoverable error while loading elf segments");
    });

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE)
        .expect("Error allocating stack");
    emu.set_reg(Register::Sp, (stack + (1024 * 1024)) - 8);
    // TODO setup argc, argv & envp

    // Setu hooks
    emu.hooks.insert(*sym_map.get("_malloc_r")
                     .expect("Inserting Malloc hook failed"), malloc_hook);
    emu.hooks.insert(*sym_map.get("_free_r")
                     .expect("Inserting Free hook failed"), free_hook);

    // Spawn worker threads to do the actual fuzzing
    for thr_id in 0..8 {
        let emu = emu.fork();
        thread::spawn(move || worker(thr_id, emu));
    }

    // Continuous statistic tracking via message passing in main thread
    let start = Instant::now();
    let mut last_time = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let stats = emu.jit.stats.lock().unwrap();
        let elapsed = start.elapsed().as_secs_f64();

        if last_time.elapsed() >= Duration::from_millis(1000) {
            println!("[{:8.2}] fuzz cases: {} ; fcps: {}", elapsed, stats.total_cases,
                     stats.total_cases  as f64 / elapsed);

            last_time = Instant::now();
        }
    }
}
