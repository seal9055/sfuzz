mod mmu;
pub mod emulator;

use elfparser;
use elfparser::{ELFMAGIC, ARCH64, LITTLEENDIAN, TYPEEXEC};
use emulator::{
    Emulator, 
    Register,
};
use std::{
    process,
};

/// Small wrapper to easily handle unrecoverable errors without panicking
pub fn error_exit(msg: &str) -> ! {
    println!("{}", msg);
    process::exit(1);
}

/// Used to verify that the binary is suitable for this fuzzer. (64-bit, ELF, Little Endian...)
fn verify_format(elf_hdr: elfparser::Header) -> Result<(), String> {
    if elf_hdr.magic   != ELFMAGIC     { return Err("Magic value does not match ELF".to_string()); }
    if elf_hdr.bitsize != ARCH64       { return Err("Architecture is not 64-bit".to_string());     }
    if elf_hdr.endian  != LITTLEENDIAN { return Err("Endian is not Little Endian".to_string());    }
    if elf_hdr.o_type  != TYPEEXEC     { return Err("Elf is not an executeable".to_string());      }
    //if elf_hdr.machine != RISCV        { return Err("Elf is not Riscv architecture".to_string());  }
    Ok(())
}

/// Parse ELF Headers and Program Headers. If all headers are valid, proceed to load each loadable
/// segment into the emulators memory space
pub fn load_elf_segments(filename: &str, emu_inst: &mut Emulator) -> Option<()> {
    let target = std::fs::read(filename).ok()?;
    let elf_hdr = elfparser::Header::new(&target)?;
    if let Err(error) = verify_format(elf_hdr) {
        error_exit(&format!("Process exited with error: {}", error));
    }
    let mut offset = elf_hdr.phoff - elf_hdr.phentsize as usize;

    // Loop through all segment and allocate memory for each segment with segment-type load
    for _ in 0..elf_hdr.phnum {
        offset += elf_hdr.phentsize as usize;
        let program_hdr = elfparser::ProgramHeader::new(&target[offset..])?;

        if program_hdr.seg_type != elfparser::LOADSEGMENT { continue; }

        emu_inst.load_segment(program_hdr, &target[program_hdr.offset..
                              program_hdr.offset.checked_add(program_hdr.memsz)?])?;
        //println!("{:#x?}", program_hdr);
    }

    emu_inst.set_reg(Register::Pc, elf_hdr.entry_addr);

    Some(())
}

/// Wrapper function for each emulator, takes care of running the emulator, memory resets, etc
pub fn worker(thr_id: usize, mut emu: Emulator) {
    let original = emu.clone();
    loop {
        emu = original.clone();
        emu.run_emu();
    }
}
