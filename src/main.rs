use sfuzz::{
    mmu::{Perms},
    emulator::{Emulator, Register},
    error_exit, load_elf_segments, worker,
    shared::{Shared},
};
use std::thread;
use std::sync::Arc;

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    let shared = Arc::new(Shared::new(16 * 1024 * 1024));
    let mut emu = Emulator::new(32 * 1024 * 1024, shared);

    if load_elf_segments("./test_bin", &mut emu).is_none() {
        error_exit("Unrecoverable error while loading elf segments");
    }

    //for hook in emu.hooks {

    //}

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE).expect("Error allocating stack");
    emu.set_reg(Register::Sp, stack + 1024 * 1024);
    // TODO

    for thr_id in 0..1 {
        let emu = emu.clone();
        thread::spawn(move || worker(thr_id, emu));
    }

    loop {}
}
