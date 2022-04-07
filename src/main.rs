use sfuzz::{
    mmu::Perms,
    emulator::{Emulator, Register, Fault},
    error_exit, load_elf_segments, worker,
    jit::{Jit, LibFuncs},
};
use std::thread;
use std::sync::Arc;
use std::time::{Duration, Instant};

use byteorder::{LittleEndian, WriteBytesExt};

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
    let sym_map = load_elf_segments("./objdump", &mut emu).unwrap_or_else(||{
        error_exit("Unrecoverable error while loading elf segments");
    });

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE)
        .expect("Error allocating stack");
    emu.set_reg(Register::Sp, (stack + (1024 * 1024)) - 8);

    // Allocate space for argv[0] & argv[1]
    let arg0 = emu.allocate(64, Perms::READ | Perms::WRITE).expect("Allocating argv[0] failed");
    let arg1 = emu.allocate(64, Perms::READ | Perms::WRITE).expect("Allocating argv[1] failed");
    emu.memory.write_mem(arg0, b"target_bin\0", 11).expect("Writing to argv[0] failed");
    emu.memory.write_mem(arg1, b"test_arg1\0", 10).expect("Writing to argv[1] failed");

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
    push!(0u64);    // Argv[2] (null to terminate array)
    push!(arg1);    // Argv[1]
    push!(arg0);    // Argv[0]
    push!(2u64);    // Argc

    // Setup hooks
    emu.hooks.insert(*sym_map.get("_malloc_r")
                     .expect("Inserting _malloc_r hook failed"), malloc_hook);

    emu.hooks.insert(*sym_map.get("xmalloc")
                     .expect("Inserting xmalloc hook failed"), malloc_hook);

    emu.hooks.insert(*sym_map.get("_free_r")
                     .expect("Inserting _free_r hook failed"), free_hook);

    emu.hooks.insert(*sym_map.get("free")
                     .expect("Inserting free hook failed"), free_hook);

    for v in &sym_map {
        println!("{:?}", v);
    }
    emu.custom_lib.insert(*sym_map.get("strlen")
                     .expect("Inserting strlen custom code failed"), LibFuncs::STRLEN);
    emu.custom_lib.insert(*sym_map.get("strcmp")
                     .expect("Inserting strlen custom code failed"), LibFuncs::STRCMP);

    // Spawn worker threads to do the actual fuzzing
    for thr_id in 0..1 {
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
