use sfuzz::{
    mmu::{Perms},
    emulator::{Emulator, Register, Fault},
    error_exit, load_elf_segments, worker,
    shared::{Shared},
};
use std::thread;
use std::sync::Arc;

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

fn free_hook(emu: &mut Emulator) -> Result<(), Fault> {
    let ptr = emu.get_reg(Register::A1);

    emu.memory.free(ptr).unwrap();
    emu.set_reg(Register::Pc, emu.get_reg(Register::Ra));
    Ok(())
}

/// Setup the root emulator's segments and stack before cloning the emulator into multiple threads
/// to run multiple emulators at the same time
fn main() {
    let shared = Arc::new(Shared::new(16 * 1024 * 1024));
    let mut emu = Emulator::new(32 * 1024 * 1024, shared);

    let sym_map = load_elf_segments("./test_bin_i", &mut emu).unwrap_or_else(||{
        error_exit("Unrecoverable error while loading elf segments");
    });

    // Setup Stack
    let stack = emu.allocate(1024 * 1024, Perms::READ | Perms::WRITE).expect("Error allocating stack");
    emu.set_reg(Register::Sp, stack + 1024 * 1024);
    // TODO

    emu.hooks.insert(*sym_map.get("_malloc_r")
                     .expect("Inserting Malloc hook failed"), malloc_hook);
    emu.hooks.insert(*sym_map.get("_free_r")
                     .expect("Inserting Free hook failed"), free_hook);

    for thr_id in 0..1 {
        let emu = emu.clone();
        thread::spawn(move || worker(thr_id, emu));
    }

    loop {}
}
