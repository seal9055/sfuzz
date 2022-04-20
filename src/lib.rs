pub mod emulator;
pub mod mmu;
pub mod riscv;
pub mod jit;
pub mod syscalls;
pub mod irgraph;
pub mod mutator;
pub mod config;

extern crate iced_x86;

use elfparser::{self, ARCH64, ELFMAGIC, LITTLEENDIAN, TYPEEXEC, RISCV};
use emulator::{Emulator, Register, Fault};
use mutator::Mutator;
use my_libs::sorted_vec::*;
use config::{CovMethod, COVMETHOD};

use std::process;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustc_hash::FxHashMap;
use rand::Rng;
use fasthash::{xx::Hash32, FastHash};
use parking_lot::RwLock;

const SAVE_CRASHES: bool = false;

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
    let mut function_listing = SortedVec::default();

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
    let mut func_names: FxHashMap<usize, String> = FxHashMap::default();

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

        /*
            // Insert a mapping from the symbol name to its address into a hashmap we are returning
            symbol_map.insert(sym_name.to_string(), sym_entry.sym_value);
        */

        // If the entry is a function, insert a mapping from the symbol name to its address into a
        // hashmap we are returning
        if sym_entry.sym_info == 0x2 || sym_entry.sym_info == 0x12 {
            symbol_map.insert(sym_name.to_string(), sym_entry.sym_value);
            function_listing.insert((sym_entry.sym_value, sym_entry.sym_size), 
                                    sym_entry.sym_value as isize);
            func_names.insert(sym_entry.sym_value, sym_name.to_string());
        }
    }

    // Some functions such as `frame_dummy` have a size of 0 listed in their metadata. This causes
    // issues once I need to use this size to determine the function end, so whenever this happens I
    // instead determine the function size using the start address of the next function.
    for i in 0..function_listing.0.len() {
        let mut v = function_listing.0[i];
        if v.1 == 0 {
            v.1 = function_listing.0[i+1].0 - v.0;
        }
        // function address, size, name
        emu_inst.functions.insert(v.0, (v.1, func_names.get(&v.0).unwrap().clone()));
    }

    emu_inst.set_reg(Register::Pc, elf_hdr.entry_addr);
    Some(symbol_map)
}


/// Holds various information related to tracking statistics for the fuzzer
#[derive(Default, Debug)]
pub struct Statistics {
    pub total_cases: usize,

    pub crashes: usize,

    pub coverage: usize,
}

#[derive(Debug, Clone)]
pub struct Input {
    /// Raw bytes stored in this input
    data: Vec<u8>,
}

impl Input {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data: data.to_vec(),
        }
    }
}

/// Structure that is meant to be shared between threads. Tracks fuzz inputs and coverage
#[derive(Debug)]
pub struct Corpus {
    /// Actual byte-backing for the fuzz-inputs
    pub inputs: RwLock<Vec<Input>>,

    /// Coverage map - used by block level coverage with hit-counter
    pub coverage_map: Option<RwLock<FxHashMap<usize, usize>>>,

    /// Coverage vector - used by block level coverage without hit-counter
    pub coverage_vec: Option<RwLock<Vec<usize>>>,

    /// Counter that keeps track of current coverage
    pub cov_counter: AtomicUsize,
}

impl Corpus {
    pub fn new(_size: usize) -> Self {
        let (coverage_vec, coverage_map) = match COVMETHOD {
            CovMethod::Block => {
                (Some(RwLock::new(Vec::new())), None)
            },
            CovMethod::BlockHitCounter => {
                (None, Some(RwLock::new(FxHashMap::default())))
            },
            _ => (None, None)
        };
        Self {
            inputs:   RwLock::new(Vec::new()),
            coverage_map,
            coverage_vec,
            cov_counter: AtomicUsize::new(0),
        }
    }
}

/// Run the emulator until a Snapshot fault is returned, at which point the injected code is 
/// overwritten with nops, and the 'advanced' emulator is returned back to main
pub fn snapshot(emu: &mut Emulator, mut corpus: Arc<Corpus>) {
    let case_res = emu.run_jit(&mut corpus);
    match case_res.0.unwrap() {
        Fault::Snapshot => {
            // Overwrite the snapshot code with nops so we dont break there again.
            emu.jit.nop_code(emu.snapshot_addr, None);
        },
        _ => panic!("Failed to reach snapshot address, make sure it is trivially reachable"),
    }
}

/// Wrapper function for each emulator, takes care of running the emulator, memory resets, etc
pub fn worker(_thr_id: usize, mut emu: Emulator, mut corpus: Arc<Corpus>, tx: Sender<Statistics>) {
    // Maintain an original copy of the passed in emulator so it can later be referenced
    let original = emu.fork();

    // Data is sent to the main thread in batches to increase throughput
    const BATCH_SIZE: usize = 100000;

    // Keep track of how many cases are being executed
    let mut count = 0;

    // Initialize rng to be used for random seed selection and other purposes
    let mut rng = rand::thread_rng();

    // Initialize a mutator that will be in charge of randomly corrupting input
    let mut mutator = Mutator::new(rng.clone());

    // Locally count the number of crashes
    let mut local_crashes = 0;

    loop {
        // Reset the emulator state
        emu.reset(&original);
        emu.fuzz_input.clear();

        // Select random seed from corpus
        let corpus_len = corpus.inputs.read().len();
        let index = rng.gen_range(0..corpus_len);
        emu.fuzz_input.extend_from_slice(&corpus.inputs.read()[index].data);

        // Mutate the previously chosen seed
        mutator.mutate(&mut emu.fuzz_input);

        // If a crash occured, save the input and increase crash count, otherwise just move on
        let case_res = emu.run_jit(&mut corpus);
        match case_res.0.unwrap() {
            Fault::ReadFault(v) => {
                let h = Hash32::hash(&emu.fuzz_input);
                let crash_dir = format!("crashes/read_{:x}_{}", v, h);
                if SAVE_CRASHES { std::fs::write(&crash_dir, &emu.fuzz_input).unwrap(); }
                local_crashes += 1;
            },
            Fault::WriteFault(v) => {
                let h = Hash32::hash(&emu.fuzz_input);
                let crash_dir = format!("crashes/write_{:x}_{}", v, h);
                if SAVE_CRASHES { std::fs::write(&crash_dir, &emu.fuzz_input).unwrap(); }
                local_crashes += 1;
            },
            Fault::OutOfBounds(v) => {
                let h = Hash32::hash(&emu.fuzz_input);
                let crash_dir = format!("crashes/oob_{:x}_{}", v, h);
                if SAVE_CRASHES { std::fs::write(&crash_dir, &emu.fuzz_input).unwrap(); }
                local_crashes += 1;
            },
            Fault::Snapshot => panic!("Hit snapshot during execution, this should not happen"),
            Fault::Exit => {},
            _ => unreachable!(),
        }

        // This input found new coverage
        if case_res.1 {
            corpus.inputs.write().push(Input::new(emu.fuzz_input.clone()));
        }

        // New case completed
        count +=1;

        // Once `count` reaches `BATCH_SIZE`, send statistics to main thread and reset count
        if count == BATCH_SIZE {
            //let mut stats = emu.jit.stats.lock().unwrap();
            //stats.total_cases += BATCH_SIZE;
            //stats.crashes += crashes;

            // Populate statistics that will be sent to the main thread
            let stats = Statistics {
                total_cases: BATCH_SIZE,
                crashes: local_crashes,
                coverage: corpus.cov_counter.load(Ordering::SeqCst),
            };

            // Send stats over to the main thread
            tx.send(stats).unwrap();

            // Reset local statistics
            count = 0;
            local_crashes = 0;
        }
    }
}
