pub mod emulator;
pub mod mmu;
pub mod riscv;
pub mod shared;
pub mod syscalls;

extern crate iced_x86;

use elfparser;
use elfparser::{ARCH64, ELFMAGIC, LITTLEENDIAN, TYPEEXEC, RISCV};
use emulator::{Emulator, Register};
use std::process;

/// Small wrapper to easily handle unrecoverable errors without panicking
pub fn error_exit(msg: &str) -> ! {
    println!("{}", msg);
    process::exit(1);
}

/// Used to verify that the binary is suitable for this fuzzer. (64-bit, ELF, Little Endian...)
fn verify_elf_hdr(elf_hdr: elfparser::Header) -> Result<(), String> {
    if elf_hdr.magic != ELFMAGIC {
        return Err("Magic value does not match ELF".to_string());
    }
    if elf_hdr.bitsize != ARCH64 {
        return Err("Architecture is not 64-bit".to_string());
    }
    if elf_hdr.endian != LITTLEENDIAN {
        return Err("Endian is not Little Endian".to_string());
    }
    if elf_hdr.o_type != TYPEEXEC {
        return Err("Elf is not an executeable".to_string());
    }
    if elf_hdr.machine != RISCV {
        return Err("Elf is not Riscv architecture".to_string());
    }
    Ok(())
}

/// Parse ELF Headers and Program Headers. If all headers are valid, proceed to load each loadable
/// segment into the emulators memory space
pub fn load_elf_segments(filename: &str, emu_inst: &mut Emulator) -> Option<()> {
    let target = std::fs::read(filename).ok()?;
    let elf_hdr = elfparser::Header::new(&target)?;
    if let Err(error) = verify_elf_hdr(elf_hdr) {
        error_exit(&format!("Process exited with error: {}", error));
    }

    // Loop through all segment and allocate memory for each segment with segment-type load
    let mut offset = elf_hdr.phoff - elf_hdr.phentsize as usize;
    for _ in 0..elf_hdr.phnum {
        offset += elf_hdr.phentsize as usize;
        let program_hdr = elfparser::ProgramHeader::new(&target[offset..])?;

        if program_hdr.seg_type != elfparser::LOADSEGMENT {
            continue;
        }

        emu_inst.load_segment(
            program_hdr,
            &target[program_hdr.offset..program_hdr.offset.checked_add(program_hdr.memsz)?],
        )?;
        //println!("{:#x?}", program_hdr);
    }

    // Loop through all section headers to find the symbol tab
    offset = elf_hdr.shoff - elf_hdr.shentsize as usize;
    let mut symtab_hdr: Option<elfparser::SectionHeader> = None;
    for _ in 0..elf_hdr.shnum {
        offset += elf_hdr.shentsize as usize;

        let section_hdr = elfparser::SectionHeader::new(&target[offset..])?;

        if section_hdr.s_type == 0x2 {
            symtab_hdr = Some(section_hdr);
        }
    }

    let symtab_hdr = symtab_hdr.unwrap();

    // Use symbol table to extract all function addresses and sizes. Our JIT can use this
    // information to determine where functions start/end
    offset = symtab_hdr.s_offset - symtab_hdr.s_entsize;
    let num_entries = symtab_hdr.s_size / symtab_hdr.s_entsize;
    for v in 0..num_entries {
        offset += symtab_hdr.s_entsize;

        let sym_entry = elfparser::SymbolTable::new(&target[offset..])?;

        // If the entry is a function (local or global), add it to function hashmap
        //if sym_entry.sym_info == 0x2 || sym_entry.sym_info == 0x12 {
        //    emu_inst.function_map.insert(sym_entry.sym_value, sym_entry.sym_size);
        //}

        if v == 270 {
            println!("hook: {:?}", sym_entry);
        }
        // Add hooks for functions we want to hook
        match sym_entry.sym_name {
            2511 => { // Hook __malloc_r
                emu_inst.hooks.push((sym_entry.sym_name, sym_entry.sym_value));
            },
            _ => {},
        }
    }
    emu_inst.set_reg(Register::Pc, elf_hdr.entry_addr);
    Some(())
}

/// Wrapper function for each emulator, takes care of running the emulator, memory resets, etc
pub fn worker(_thr_id: usize, mut emu: Emulator) {
    let original = emu.clone();
    loop {
        emu = original.clone();

        let exit_reason = emu.run_jit().unwrap();
        println!("Exit Reason is: {:?}", exit_reason);
    }
}
