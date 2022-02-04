pub mod emulator;
pub mod mmu;
pub mod riscv;
pub mod jit;
pub mod syscalls;
pub mod irgraph;
pub mod cfg;

#[macro_use] extern crate maplit;
extern crate iced_x86;

use elfparser::{self, ARCH64, ELFMAGIC, LITTLEENDIAN, TYPEEXEC, RISCV};
use emulator::{Emulator, Register};
use std::process;
use rustc_hash::FxHashMap;

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
/// segment into the emulators memory space and extracts symbol table entries which are then
/// returned via a hashmap
pub fn load_elf_segments(filename: &str, emu_inst: &mut Emulator)
        -> Option<FxHashMap<String, usize>> {
    let target = std::fs::read(filename).ok()?;
    let elf_hdr = elfparser::Header::new(&target)?;
    let mut symbol_map: FxHashMap<String, usize> = FxHashMap::default();

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
    }

    // Loop through all section headers to extract the symtab and the strtab
    offset = elf_hdr.shoff - elf_hdr.shentsize as usize;
    let mut symtab_hdr: Option<elfparser::SectionHeader> = None;
    let mut strtab_hdr: Option<elfparser::SectionHeader> = None;

    for i in 0..elf_hdr.shnum {
        offset += elf_hdr.shentsize as usize;

        let section_hdr = elfparser::SectionHeader::new(&target[offset..])?;

        if section_hdr.s_type == 0x2 {
            symtab_hdr = Some(section_hdr);
        } else if section_hdr.s_type == 0x3 && i != elf_hdr.shstrndx {
            strtab_hdr = Some(section_hdr);
        }
    }

    let symtab_hdr = symtab_hdr.unwrap();
    let strtab_off = strtab_hdr.unwrap().s_offset;

    // Use symbol table to extract all symbol names and addresses. Our JIT can use this
    // information to place hooks at specific function entries
    offset = symtab_hdr.s_offset - symtab_hdr.s_entsize;
    let num_entries = symtab_hdr.s_size / symtab_hdr.s_entsize;

    for _ in 0..num_entries {
        offset += symtab_hdr.s_entsize;
        let sym_entry = elfparser::SymbolTable::new(&target[offset..])?;

        // Extract names for symbol table entry from the strtab
        let str_start = strtab_off+sym_entry.sym_name as usize;
        let str_size  = (&target[str_start..]).iter().position(|&b| b == 0).unwrap_or(target.len());
        let sym_name = std::str::from_utf8(&target[str_start..str_start + str_size]).unwrap_or("");

        // If the entry is a function, insert a mapping from the symbol name to its address into a
        // hashmap we are returning
        if sym_entry.sym_info == 0x2 || sym_entry.sym_info == 0x12 {
            emu_inst.functions.insert(sym_entry.sym_value, sym_entry.sym_size);
            symbol_map.insert(sym_name.to_string(), sym_entry.sym_value);
        }
    }
    emu_inst.set_reg(Register::Pc, elf_hdr.entry_addr);
    Some(symbol_map)
}

/// Wrapper function for each emulator, takes care of running the emulator, memory resets, etc
pub fn worker(_thr_id: usize, mut emu: Emulator) {
    let original = emu.clone();
    const BATCH_SIZE: usize = 10;
    let mut count = 0;
    loop {
        emu.reset(&original); //= original.fork();
        emu.run_jit().unwrap();
        count +=1;
        if count == BATCH_SIZE {
            count = 0;
            emu.jit.stats.lock().unwrap().total_cases += BATCH_SIZE; }
    }
}
