//! # SFUZZ
//!
//! This is a performance and scaling focused blackbox fuzzer that makes use of RISC-V to x86_64 
//! binary translations and a custom JIT-compilation engine. The fuzzer is still in development, 
//! but currently successfuly runs against simple targets. 

#![feature(once_cell)]

pub mod emulator;
pub mod mmu;
pub mod riscv;
pub mod jit;
pub mod syscalls;
pub mod irgraph;
pub mod mutator;
pub mod config;
pub mod pretty_printing;

extern crate iced_x86;

use elfparser::{self, ARCH64, ELFMAGIC, LITTLEENDIAN, TYPEEXEC, RISCV};
use emulator::{Emulator, Register, Fault};
use mutator::Mutator;
use my_libs::sorted_vec::*;
use config::{FULL_TRACE, OUTPUT_DIR};

use std::process;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::arch::asm;

use rustc_hash::FxHashMap;
use fasthash::{xx::Hash32, FastHash};
use parking_lot::RwLock;

const SAVE_CRASHES: bool = true;

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
    /// Total number of fuzz cases
    pub total_cases: usize,

    /// Total crashes
    pub crashes: usize,

    /// Unique crashes
    pub ucrashes: usize,

    /// Number of coverage units that has been reached
    pub coverage: usize,

    /// Number of instructions executed
    pub instr_count: u64,

    /// How often a fuzz-input times out due to taking too long
    pub timeouts: u64,
}

#[derive(Debug, Clone)]
pub struct Input {
    /// Raw byte backing of this input
    data: Vec<u8>,

    /// Size of this input
    size: usize,

    /// Execution time of this input (measured using the amount of instructions this input executed
    /// since using a syscall to get the actual time is very expensive). Calibrated on startup for
    /// initial seeds
    exec_time: Option<u64>,

    /// Counter incremented whenever this case hits new coverage
    cov_finds: usize,

    /// Counter incremented whenever a mutation on this case finds a crash. Frequent crashes
    /// reduce energy
    crashes: usize,

    /// Counter incremented whenever a mutation on this case finds a new crash. Unlike similar
    /// crashes, new unique crashes increase a cases energy
    ucrashes: usize,
}

impl Input {
    pub fn new(data: Vec<u8>, exec_time: Option<u64>) -> Self {
        Self {
            data: data.to_vec(),
            size: data.len(),
            exec_time,
            cov_finds: 0,
            crashes: 0,
            ucrashes: 0,
        }
    }

    pub fn calculate_energy(&self, corpus: &Corpus) -> usize {
        let mut energy: isize = 80000;
        let num_inputs = corpus.inputs.read().len();
        let avg_size = (corpus.total_size.load(Ordering::SeqCst) / num_inputs) as isize;
        let avg_exec_time = (corpus.total_exec_time.load(Ordering::SeqCst) / num_inputs) as isize;

        // Calculate if this case is sized above or below average, and by how much, and use this
        // to change the cases energy. Shorter cases have their enegry increased
        let size_diff: isize = (self.size as isize) - avg_size;
        let size_diff_percentage: f64 = (size_diff as f64) / (avg_size as f64);
        energy = energy.saturating_add((size_diff_percentage * 100000f64) as isize);
        
        // Calculate if this case's execution time is above or below average, and by how much, and
        // use this to change the cases energy. Faster cases have their enegry increased
        let runtime_diff: isize = (self.exec_time.unwrap() as isize) - avg_exec_time;
        let runtime_diff_percentage: f64 = (runtime_diff as f64) / (avg_exec_time as f64);
        energy = energy.saturating_add((runtime_diff_percentage * 100000f64) as isize);
        
        // For every instance of this case finding new coverage, increase energy by ~10%
        for _ in 0..self.cov_finds {
            energy += energy / 10;
        }

        // For every instance of this case finding a new unique crash, increase energy by ~10%
        for _ in 0..self.ucrashes {
            energy += energy / 10;
        }

        // Reduce energy if this case has already found a massive amount of crashes since it might
        // be permanently stuck on a similarly crashing path
        energy = energy.saturating_sub(self.crashes as isize);

        // Make sure energy remains in the 20000 - 150000 range
        energy = core::cmp::max(energy, 20000);
        energy = core::cmp::min(energy, 150000);
        energy as usize
    }
}

/// Structure that is meant to be shared between threads. Tracks fuzz inputs and coverage
#[derive(Debug)]
pub struct Corpus {
    /// Actual byte-backing for the fuzz-inputs
    pub inputs: RwLock<Vec<Input>>,

    /// Bytemap used in jits to determine if an edge has already been hit
    pub coverage_bytemap: Vec<usize>,

    /// Counter that keeps track of current coverage
    pub cov_counter: AtomicUsize,

    /// Used to dedup crashses and only save off unique crashes
    pub crash_mapping: RwLock<FxHashMap<Fault, u8>>,

    /// Total size of the inputs in this corpus
    pub total_size: AtomicUsize,

    /// Total execution time of the inputs in this corpus
    pub total_exec_time: AtomicUsize,
}

impl Corpus {
    /// Start a new corpus. Initialize fields based on what type of coverage method is in use.
    pub fn new(size: usize) -> Self {
        Self {
            inputs:           RwLock::new(Vec::new()),
            coverage_bytemap: vec![0; size],
            cov_counter:      AtomicUsize::new(0),
            crash_mapping:    RwLock::new(FxHashMap::default()),
            total_size:       AtomicUsize::new(0),
            total_exec_time:  AtomicUsize::new(0),
        }
    }

    /// During initial calibration coverage is gained without being tracked for the statistics, so
    /// the coverage map is reset after initial calibration to more accurately represent collected
    /// coverage
    pub fn reset_coverage(&mut self) {
        self.coverage_bytemap = vec![0; self.coverage_bytemap.len()];
        self.cov_counter = AtomicUsize::new(0);
    }
}

/// Run the emulator until a Snapshot fault is returned, at which point the injected code is
/// overwritten with nops, and the 'advanced' emulator is returned back to main
pub fn snapshot(emu: &mut Emulator, corpus: &Corpus) {
    // Setup data-structures for tracing, unnecessary for calibration, but required for run_jit
    // function
    let mut trace_arr: Vec<u64> = if *FULL_TRACE.get().unwrap() {
        vec![0u64; 1024 * 1024 * 64]
    } else {
        Vec::new()
    };
    let mut trace_arr_len: usize = 0;
    let mut tmp = 0;

    // Run jit until finish and collect how long this input needed
    let case_res = emu.run_jit(corpus, &mut tmp, &mut trace_arr, &mut trace_arr_len);
    match case_res.0.unwrap() {
        Fault::Snapshot => {
            // Overwrite the snapshot code with nops so we dont break there again.
            emu.jit.nop_code(emu.snapshot_addr, None);
            println!("Snapshot taken");

        },
        _ => panic!("Failed to reach snapshot address, make sure it is trivially reachable"),
    }
}

/// Callibrate how long the initial seeds take to run and use it to determine timeout
pub fn calibrate_seeds(emu: &mut Emulator, corpus: &Corpus) -> u64 {
    let num_inputs = corpus.inputs.read().len();
    let mut instr_count = 0;

    let original = emu.fork();
    let mut avg: u64 = 0;

    for i in 0..num_inputs {
        emu.fuzz_input.extend_from_slice(&corpus.inputs.read()[i].data);

        // Setup data-structures for tracing, unnecessary for calibration, but required for run_jit
        // function
        let mut trace_arr: Vec<u64> = if *FULL_TRACE.get().unwrap() {
            vec![0u64; 1024 * 1024 * 64]
        } else {
            Vec::new()
        };
        let mut trace_arr_len: usize = 0;

        // Run jit until finish and collect how long this input needed
        emu.run_jit(corpus, &mut instr_count, &mut trace_arr, &mut trace_arr_len);

        let mut inputs = corpus.inputs.write();
        inputs[i].exec_time = Some(instr_count);

        avg += instr_count;

        emu.fuzz_input.clear();
        emu.reset(&original);
        instr_count = 0;
    }

    // Timeout is the average initial seed execution time * 5
    (avg / num_inputs as u64) * 5
}

/// Emit trace for the entire program execution. This is formatted the same way as gdb + qemu's
/// `info register` command, so diff files can be generated for debugging purposes. (Some slight
/// output formatting on gdb's part is required, and stack addresses will differ between this jit
/// and qemu
fn emit_trace(trace_arr: &[u64], trace_arr_len: usize) {
    let mut i = 0;
    let mut s = Vec::new();
    println!("emitting trace...");
    while i < trace_arr_len {
        s.push(format!("ra 0x{:x}", trace_arr[i+1]));
        s.push(format!("sp 0x{:x}", trace_arr[i+2]));
        s.push(format!("gp 0x{:x}", trace_arr[i+3]));
        s.push(format!("tp 0x{:x}", trace_arr[i+4]));
        s.push(format!("t0 0x{:x}", trace_arr[i+5]));
        s.push(format!("t1 0x{:x}", trace_arr[i+6]));
        s.push(format!("t2 0x{:x}", trace_arr[i+7]));
        s.push(format!("fp 0x{:x}", trace_arr[i+8]));
        s.push(format!("s1 0x{:x}", trace_arr[i+9]));
        s.push(format!("a0 0x{:x}", trace_arr[i+10]));
        s.push(format!("a1 0x{:x}", trace_arr[i+11]));
        s.push(format!("a2 0x{:x}", trace_arr[i+12]));
        s.push(format!("a3 0x{:x}", trace_arr[i+13]));
        s.push(format!("a4 0x{:x}", trace_arr[i+14]));
        s.push(format!("a5 0x{:x}", trace_arr[i+15]));
        s.push(format!("a6 0x{:x}", trace_arr[i+16]));
        s.push(format!("a7 0x{:x}", trace_arr[i+17]));
        s.push(format!("s2 0x{:x}", trace_arr[i+18]));
        s.push(format!("s3 0x{:x}", trace_arr[i+19]));
        s.push(format!("s4 0x{:x}", trace_arr[i+20]));
        s.push(format!("s5 0x{:x}", trace_arr[i+21]));
        s.push(format!("s6 0x{:x}", trace_arr[i+22]));
        s.push(format!("s7 0x{:x}", trace_arr[i+23]));
        s.push(format!("s8 0x{:x}", trace_arr[i+24]));
        s.push(format!("s9 0x{:x}", trace_arr[i+25]));
        s.push(format!("s10 0x{:x}", trace_arr[i+26]));
        s.push(format!("s11 0x{:x}", trace_arr[i+27]));
        s.push(format!("t3 0x{:x}", trace_arr[i+28]));
        s.push(format!("t4 0x{:x}", trace_arr[i+29]));
        s.push(format!("t5 0x{:x}", trace_arr[i+30]));
        s.push(format!("t6 0x{:x}", trace_arr[i+31]));
        s.push(format!("pc 0x{:x}", trace_arr[i+32]));
        s.push(String::new());

        let v = s.join("\n");
        std::fs::write("trace", v).unwrap();
        i+=33;
    }
}

/// Wrapper function for each emulator, takes care of running the emulator, memory resets, etc
pub fn worker(_thr_id: usize, mut emu: Emulator, corpus: Arc<Corpus>, tx: Sender<Statistics>) {
    // Maintain an original copy of the passed in emulator so it can later be referenced
    let original = emu.fork();

    // Initialize a mutator that will be in charge of randomly corrupting input
    let mut mutator = Mutator::default();

    // Locally count the number of crashes, total and unique
    let mut local_total_crashes = 0;
    let mut local_unique_crashes = 0;
    let mut local_coverage_count = 0;
    let mut local_instr_count = 0;
    let mut local_timeouts = 0;

    // Current index into the input array of the corpus
    let mut input_index = 0;

    // Allocated memory to trace the jit execution
    let mut first_trace: bool = true;
    let mut trace_arr: Vec<u64> = if *FULL_TRACE.get().unwrap() {
        vec![0u64; 1024 * 1024 * 64]
    } else {
        Vec::new()
    };

    // Save callee saved registers so that they can later be restored. This shouldn't really matter
    // while fuzzing since this function should never return, but still good to have
    let mut callee_saved_regs = vec![0u64; 8];
    unsafe {
        asm!(r#"
            mov [{0}], rbx
            mov [{0} + 0x8], rsp
            mov [{0} + 0x10], rbp
            mov [{0} + 0x18], r12
            mov [{0} + 0x20], r13
            mov [{0} + 0x28], r14
            mov [{0} + 0x30], r15
        "#,
        in(reg) callee_saved_regs.as_mut_ptr(),
        );
    }

    loop {
        // Get the next seed from the input queue and calculate its energy. This enery is then used
        // to determine how often this input should be run before moving on to the next input
        //input_index = corpus.select_seed(input_index, &mut rng).unwrap();
        input_index = (input_index + 1) % corpus.inputs.read().len();
        let seed_energy = corpus.inputs.read()[input_index].calculate_energy(&corpus);

        for _ in 0..seed_energy {
            // Reset the emulator state
            emu.reset(&original);
            emu.fuzz_input.clear();

            emu.fuzz_input.extend_from_slice(&corpus.inputs.read()[input_index].data);

            // Mutate the previously chosen seed
            mutator.mutate(&mut emu.fuzz_input);

            // Execute actual fuzz case and save off status
            let mut case_instr_count: u64 = 0;
            let mut trace_arr_len: usize = 0;
            let case_res = emu.run_jit(&corpus, &mut case_instr_count, &mut trace_arr,
                                       &mut trace_arr_len);

            // Write out a trace on the first fuzz case if requested
            if *FULL_TRACE.get().unwrap() && first_trace {
                first_trace = false;
                emit_trace(&trace_arr, trace_arr_len);
            }

            match case_res.0.unwrap() {
                // This means that a crash is found. Determine if the crash is unique, and if so,
                // save it.
                Fault::ReadFault(v)   |
                Fault::WriteFault(v)  |
                Fault::ExecFault(v)   |
                Fault::OutOfBounds(v) => {
                    let mut crash_map = corpus.crash_mapping.write();
                    if crash_map.get(&case_res.0.unwrap()).is_none() {
                        crash_map.insert(case_res.0.unwrap(), 0);
                        let h = Hash32::hash(&emu.fuzz_input);
                        let crash_file = match case_res.0.unwrap() {
                            Fault::ReadFault(_)   => {
                                format!("{}/crashes/read_{:x}_{}", OUTPUT_DIR.get().unwrap(), v, h)
                            },
                            Fault::WriteFault(_)   => {
                                format!("{}/crashes/write_{:x}_{}", OUTPUT_DIR.get().unwrap(), v, h)
                            },
                            Fault::ExecFault(_)   => {
                                format!("{}/crashes/exec_{:x}_{}", OUTPUT_DIR.get().unwrap(), v, h)
                            },
                            Fault::OutOfBounds(_)   => {
                                format!("{}/crashes/oob_{:x}_{}", OUTPUT_DIR.get().unwrap(), v, h)
                            },
                            _ => unreachable!(),
                        };
                        if SAVE_CRASHES { std::fs::write(&crash_file, &emu.fuzz_input).unwrap(); }
                        local_unique_crashes += 1;
                    }
                    local_total_crashes += 1;
                },
                Fault::Timeout => local_timeouts += 1,
                Fault::Snapshot => panic!("Hit snapshot during execution, this should not happen"),
                Fault::Exit => {},
                _ => unreachable!(),
            }

            // This input found new coverage
            if case_res.1 > 0 {
                let mut corp_inputs = corpus.inputs.write();
                corp_inputs[input_index].cov_finds += 1;
                local_coverage_count += case_res.1;
                corp_inputs.push(Input::new(emu.fuzz_input.clone(), Some(case_instr_count)));

                // Add this case's stats to an overall pool that is used to average these values
                // and calculate the energy for each case.
                corpus.total_size.fetch_add(emu.fuzz_input.len(), Ordering::SeqCst);
                corpus.total_exec_time.fetch_add(case_instr_count as usize, Ordering::SeqCst);
            }
            local_instr_count += case_instr_count;
        }

        // Increment crash counter for this case
        let mut corp_inputs = corpus.inputs.write();
        corp_inputs[input_index].crashes += local_total_crashes;
        corp_inputs[input_index].ucrashes += local_unique_crashes;

        // Populate statistics that will be sent to the main thread
        let stats = Statistics {
            total_cases: seed_energy,
            crashes:     local_total_crashes,
            ucrashes:    local_unique_crashes,
            coverage:    local_coverage_count,
            instr_count: local_instr_count,
            timeouts:    local_timeouts,
        };

        // Send stats over to the main thread
        tx.send(stats).unwrap();

        // Reset local statistics
        local_total_crashes = 0;
        local_unique_crashes = 0;
        local_coverage_count = 0;
        local_instr_count = 0;
        local_timeouts = 0;
    }

    // Restore callee saved registers before returning
    #[allow(unreachable_code)]
    unsafe {
        asm!(r#"
            mov rbx, [{0}]
            mov rsp, [{0} + 0x8]
            mov rbp, [{0} + 0x10]
            mov r12, [{0} + 0x18]
            mov r13, [{0} + 0x20]
            mov r14, [{0} + 0x28]
            mov r15, [{0} + 0x30]
        "#,
        in(reg) callee_saved_regs.as_ptr(),
        );
    }
}
