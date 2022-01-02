use sfuzz::emulator::{Emulator, Register};
use sfuzz::{error_exit, load_elf_segments, worker};
use std::thread;

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    let mut emu_inst = Emulator::new(32 * 1024 * 1024);

    if load_elf_segments("./test2", &mut emu_inst).is_none() {
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

    loop {}
}
