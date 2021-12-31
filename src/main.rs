use sfuzz::{load_elf_segments, error_exit};
use sfuzz::{
    emulator::{
        Emulator,
        Register,
    }
};
use std::thread;
use sfuzz::worker;

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    let mut emu_inst = Emulator::new(32 * 1024 * 1024);

    if load_elf_segments("./test_bin", &mut emu_inst).is_none() {
        error_exit("Unrecoverable error while loading process");
    }

    // Setup Stack
    let stack = emu_inst.allocate(1024 * 1024).expect("Error allocating stack");
    emu_inst.set_reg(Register::Sp, stack + 1024 * 1024);
    // TODO


    for thr_id in 0..1 {
        let emu = emu_inst.clone();
        thread::spawn(move || worker(thr_id, emu));
    }

    loop {
    }
}
