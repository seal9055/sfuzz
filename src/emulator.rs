use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    jit::{Jit, LibFuncs, CompileInputs},
    irgraph::{IRGraph, Flag},
    emulator::FileType::{STDIN, STDOUT, STDERR},
    pretty_printing::{LogType, log},
    config::NUM_THREADS,
    syscalls, Corpus, error_exit,
};

use std::sync::{Arc, Mutex};
use std::arch::asm;
use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use iced_x86::code_asm::*;

/// 33 RISCV Registers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[repr(usize)]
pub enum Register {
    Zero = 0,
    Ra,
    Sp,
    Gp,
    Tp,
    T0,
    T1,
    T2,
    S0,
    S1,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    T3,
    T4,
    T5,
    T6,
    Pc,
}

impl Register {
    pub fn is_spilled(&self) -> bool {
        true
    }

    pub fn convert_64(&self) -> AsmRegister64 {
        rax
    }

    pub fn convert_32(&self) -> AsmRegister32 {
        eax
    }

    pub fn get_offset(&self) -> u64 {
        *self as u64 * 8
    }
}

impl From<u32> for Register {
    fn from(val: u32) -> Self {
        assert!(val < 33);
        unsafe {
            core::ptr::read_unaligned(&(val as usize) as *const usize as *const Register)
        }
    }
}

/// Various faults that can occur during program execution. These can be syscalls, bugs, or other
/// non-standard behaviors that require kernel involvement
#[derive(Hash, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Syscall
    Syscall,

    /// Fault occurs when an attempt is made to write to an address without Perms::WRITE set
    WriteFault(usize),

    /// Fault occurs when an attempt is made to read from an address without Perms::READ set
    ReadFault(usize),

    /// Fault occurs when an attempt is made to execute an invalid instruction
    ExecFault(usize),

    /// A memory request went completely out of bounds
    OutOfBounds(usize),

    /// Fault occurs when some operation results in an integer overflow
    IntegerOverflow,

    /// Fault occurs when a free fails. Occurs when an invalid address is attempted to be free'd
    /// or when a free is used on a chunk of memory that does not contain the Perms::ISALLOC
    /// permission in its metadata
    InvalidFree(usize),

    /// Fault occurs when there is no more room to service new allocations
    OOM,

    /// Process called exit
    Exit,

    /// Snapshot taken for deterministic fuzzing
    Snapshot,

    /// Fuzz case needed too much time and timed out
    Timeout,
}

/// Different types of exit-conditions for the fuzzer
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ExitType {
    /// Leave JIT to create snapshot at this address
    Snapshot,

    /// Leave JIT reporting success-case
    Success,

    /// Exit JIT as if exit() was called
    Exit,
}

/// Different types of files that the fuzzer supports
#[derive(Copy, Debug, Clone, Eq, PartialEq)]
pub enum FileType {
    /// STDIN (0)
    STDIN,

    /// STDOUT (1), basically ignored apart from debug-prints to console
    STDOUT,

    /// STDERR (2), basically ignored apart from debug-prints to console
    STDERR,

    /// The input we are fuzzing. It keeps its byte-backing in emulator.fuzz_input
    FUZZINPUT,

    /// A standard file that is not 0/1/2 or the input we are fuzzing
    OTHER,

    /// Invalid file
    INVALID,
}

/// Memoery mapped file implementation
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
    /// Filetype of this file
    pub ftype:   FileType,

    /// The byte-backing used by this file. Not required by 0/1/2, or the fuzzinput
    pub backing: Option<Vec<u8>>,

    /// Cursor is used by the fuzz-input and potential other files that aren't 0/1/2
    pub cursor:  Option<usize>,
}

impl File {
    fn new(ftype: FileType) -> Self {
        let (backing, cursor) = match ftype {
            FileType::OTHER => (Some(Vec::new()), Some(0)),
            FileType::FUZZINPUT => (None, Some(0)),
            _ => (None, None),
        };
        File {
            ftype,
            backing,
            cursor,
        }
    }
}

/// Emulator that runs the actual code. Each thread gets its own emulator in which everything is
/// separate except the jit backing that all emulators share.
#[derive(Clone)]
pub struct Emulator {
    /// Memory backing for the emulator, contains actual memory bytes and permissions
    pub memory: Mmu,

    /// The register-state of this emulator
    pub regs: [usize; 33],

    /// These are used to hook specific addresses. Can be used for debug purposes or to redirect
    /// important functions such as malloc/free to our own custom implementations
    pub hooks: FxHashMap<usize, fn(&mut Emulator) -> Result<(), Fault>>,

    /// Another form of hook, just that in this case they point to precompiled code so the JIT can
    /// immediately jump to them instead of dealing with the performance overhead of hooks
    pub custom_lib: FxHashMap<usize, LibFuncs>,

    /// List of file descriptors that the process can use for syscalls
    pub fd_list: Vec<File>,

    /// Mapping between function address and function size
    pub functions: FxHashMap<usize, (usize, String)>,

    /// The actual jit compiler backing
    pub jit: Arc<Jit>,

    /// The fuzz input that is in use by the current case
    pub fuzz_input: Vec<u8>,

    /// Map of exit conditions that would cause the fuzzer to prematurely exit
    pub exit_conds: FxHashMap<usize, ExitType>,

    /// JIT-backing-address at which the injected code for the snapshot is located
    pub snapshot_addr: usize,

    /// If a fuzz case reaches this amount of instructions it will be manually terminated
    pub timeout: u64,

    /// Thread-shared mutex that is used to lock compilation so that only one thread can compile
    /// code at a time
    pub prevent_rc: Arc<Mutex<usize>>,
}

impl Emulator {
    /// Create a new emulator that has access to the shared jit backing
    pub fn new(size: usize, jit: Arc<Jit>, prevent_rc: Arc<Mutex<usize>>) -> Self {
        Emulator {
            memory:     Mmu::new(size),
            regs:       [0; 33],
            hooks:      FxHashMap::default(),
            custom_lib: FxHashMap::default(),
            fd_list:    vec![File::new(STDIN), File::new(STDOUT), File::new(STDERR)],
            functions:  FxHashMap::default(),
            jit,
            fuzz_input: Vec::new(),
            exit_conds: FxHashMap::default(),
            snapshot_addr: 0,
            timeout: 0xffffffffffffffff,
            prevent_rc,
        }
    }

    /// Fork an emulator into a new one, basically creating an exact copy
    #[must_use]
    pub fn fork(&self) -> Self {
        Emulator {
            memory:     self.memory.fork(),
            regs:       self.regs,
            hooks:      self.hooks.clone(),
            custom_lib: self.custom_lib.clone(),
            fd_list:    self.fd_list.clone(),
            functions:  self.functions.clone(),
            jit:        self.jit.clone(),
            fuzz_input: self.fuzz_input.clone(),
            exit_conds: self.exit_conds.clone(),
            snapshot_addr: self.snapshot_addr,
            timeout: self.timeout,
            prevent_rc: self.prevent_rc.clone(),
        }
    }

    /// Reset the entire state of this emulator (memory & registers)
    pub fn reset(&mut self, original: &Self) {
        self.memory.reset(&original.memory);
        self.regs = original.regs;

        self.fd_list = original.fd_list.clone();
        self.prevent_rc = original.prevent_rc.clone();
    }

    /// Allocate a new file in the emulator
    pub fn alloc_file(&mut self, ftype: FileType) -> usize {
        let file = File::new(ftype);
        self.fd_list.push(file);
        self.fd_list.len() - 1
    }

    /// Set a register
    pub fn set_reg(&mut self, reg: Register, val: usize) {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.regs[reg as usize] = val;
    }

    /// Get the value stored in a register
    pub fn get_reg(&self, reg: Register) -> usize {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { return 0; }
        self.regs[reg as usize]
    }

    /// Load a segment from the elf binary into the emulator memory
    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        self.memory.load_segment(segment, data)
    }

    /// Allocate a region of memory in the emulator
    pub fn allocate(&mut self, size: usize, perms: u8) -> Option<usize> {
        self.memory.allocate(size, perms)
    }

    /// Free a previously allocated memory region
    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        self.memory.free(addr)
    }

    /// Runs the jit until exit/crash. It checks if the code at `pc` has already been compiled. If
    /// not it starts by initiating the procedure to compile the code. At this point it has the
    /// jitcache address of where `pc` is jit compiled too. Next it sets up various arguments and
    /// calls this code. These arguments point to structures in memory that the jit-code needs to
    /// convert original addresses to their corresponding jit addresses.
    /// Once the jit exits it collects the reentry_pc (where to continue execution), and the exit
    /// code. It performs an appropriate operation based on the exit code and then continues with
    /// the loop to reenter the jit.
    pub fn run_jit(&mut self, corpus: &Corpus, instr_count: &mut u64, trace_arr: &mut [u64], 
                   trace_arr_len: &mut usize) -> (Option<Fault>, usize, usize) {
        // Extra space when the available registers are not enough to pass sufficient 
        // information in/out of the jit
        let mut scratchpad = [
            // 0 - 0x00 - Used to extract snapshot addr
            0usize,

            // 1 - 0x08 - Free
            0usize,

            // 2 - 0x10 - CmpCov bitmap
            corpus.cmpcov_bitmap.as_ptr() as usize,

            // 3 - 0x18 - CmpCov counter
            0usize,

            // 4 - 0x20 - Trace
            trace_arr.as_mut_ptr() as usize,

            // 5 - 0x28 Trace Size
            0usize,

            // 6 - 0x30 - Coverage byte-map
            corpus.coverage_bytemap.as_ptr() as usize,

            // 7 - 0x38 - Accumulated coverage hash of current input
            0usize,

            // 8 - 0x40 - Previous block
            0usize,

            // 9 - 0x48 - Coverage counter
            0usize,

            // 10 - 0x50 - Used by coverage event, address that needs to be overwritten with a 1 to
            // indicate that the coverage event has already been hit
            0usize,
        ];

        loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return (Some(Fault::ExecFault(pc)), scratchpad[9], scratchpad[3]); }

            // Determine address of the jit-backing code for the current function, either by lookup,
            // or by compiling the function if it hasn't yet been compiled
            let jit_addr = match (*self.jit).lookup(pc, None) {
                Option::None => {
                    // IR instructions + labels at start of each control block
                    let irgraph = self.lift_func(pc).unwrap();

                    let leader_set: FxHashMap<usize, usize> = irgraph.get_leaders();

                    let mut inputs: CompileInputs = CompileInputs {
                        mem_size: self.memory.memory.len(),
                        leaders: leader_set,
                        exit_conds: &mut self.exit_conds,
                        timeout: &self.timeout,
                    };

                    // Compile the previously lifted function. The lock is shared between all
                    // threads and ensures that only one thread can compile code & insert it into
                    // the shared JIT mapping at a time. 
                    //
                    // It is done this way instead of locking the entire JIT so that the threads
                    // can still all access the JIT backing without issues or locks as long as they
                    // don't need to compile new code.
                    let mut v = self.prevent_rc.lock().unwrap();
                    let ret = self.jit.compile(&irgraph, &self.hooks, &self.custom_lib, 
                                               &mut inputs);
                    *v += 1;
                    ret.unwrap()
                },
                Some(addr) => addr
            };

            let exit_code:  usize;
            let reentry_pc: usize;

            // Invoke the JIT with appropriate arguments, push/pop rbx because it is being
            // clobbered in the JIT and llvm requires it for its operations
            unsafe {
                let func = *(&jit_addr as *const usize as *const fn());

                asm!(r#"
                    push rbx
                    call {call_dest}
                    pop rbx
                "#,
                call_dest = in(reg) func,
                out("rax")   exit_code,
                out("rcx")   reentry_pc,
                inout("rsi") *instr_count,
                in("r8")     scratchpad.as_mut_ptr(),
                inout("r9")  self.memory.dirty_size,
                in("r10")    self.memory.dirty.as_ptr() as u64,
                in("r11")    self.memory.dirty_bitmap.as_ptr() as u64,
                in("r12")    self.memory.permissions.as_ptr() as u64,
                in("r13")    self.memory.memory.as_ptr() as u64,
                in("r14")    self.regs.as_ptr() as u64,
                in("r15")    self.jit.lookup_arr.as_ptr() as u64,
                );

                self.memory.dirty.set_len(self.memory.dirty_size as usize);
            }

            self.set_reg(Register::Pc, reentry_pc);
            *trace_arr_len = scratchpad[5];

            // Take action based on the exit code returned by JIT
            match exit_code {
                1 => { /* Nothing special, just need to compile next code block */ },
                2 => { /* SYSCALL */
                    match self.get_reg(Register::A7) {
                        57 => {
                            syscalls::close(self);
                        },
                        62 => {
                            syscalls::lseek(self);
                        },
                        63 => {
                            syscalls::read(self);
                        },
                        64 => {
                            syscalls::write(self);
                        },
                        80 => {
                            syscalls::fstat(self);
                        },
                        93 => {
                            return (syscalls::exit(), scratchpad[9], scratchpad[3]);
                        },
                        169 => {
                            syscalls::gettimeofday(self);
                        },
                        214 => {
                            syscalls::brk(self);
                        },
                        1024 => {
                            syscalls::open(self);
                        },
                        _ => { panic!("Unimplemented syscall: {}", self.get_reg(Register::A7)); }
                    }
                },
                3 => { /* Hooked function */
                    if let Some(callback) = self.hooks.get(&reentry_pc) {
                        match callback(self) {
                            Err(v) => return (Some(v), scratchpad[9], scratchpad[3]),
                            _ => {},
                        }
                    } else {
                        error_exit("Attempted to hook invalid function");
                    }
                },
                5 => { /* JIT exited to setup a snapshot */
                    self.snapshot_addr = scratchpad[0];
                    return (Some(Fault::Snapshot), scratchpad[9], scratchpad[3]);
                },
                7 => { /* Fuzz case timed out */
                    return (Some(Fault::Timeout), scratchpad[9], scratchpad[3]);
                },
                8 => { /* Attempted to read memory without read permissions */
                    return (Some(Fault::ReadFault(reentry_pc)), scratchpad[9], scratchpad[3]);
                },
                9 => { /* Attempted to write to memory without write permissions */
                    return (Some(Fault::WriteFault(reentry_pc)), scratchpad[9], scratchpad[3]);
                },
                10 => { /* Memory read/write request went completely out of bounds */
                    return (Some(Fault::OutOfBounds(reentry_pc)), scratchpad[9], scratchpad[3]);
                },
                _ => panic!("Invalid JIT return code: {:x}", exit_code),
            }
        }
    }

    /// Returns a BTreeMap of pc value's at which a label should be created
    fn extract_labels(&self, mut pc: usize, instrs: &[Instr]) -> BTreeMap<usize, u8> {
        let mut ret = BTreeMap::new();

        for instr in instrs {
            match instr {
                Instr::Jal { rd: _, imm} => {
                    ret.insert((pc as i32 + imm) as usize, 0);
                },
                Instr::Beq  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bne  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Blt  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bge  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bltu { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bgeu { rs1: _, rs2: _, imm, mode: _ } => {
                    ret.insert((pc as i32 + imm) as usize, 0);
                    ret.insert((pc as i32 + 4) as usize, 0);
                },
                _ => {},
            }
            pc += 4;
        }
        ret
    }

    /// Lift a function into an intermediate representation using the lift helper function
    fn lift_func(&self, mut pc: usize) -> Result<IRGraph, ()> {
        let mut irgraph = IRGraph::default();
        let mut instrs: Vec<Instr> = Vec::new();

        let start_pc = pc;
        let end_pc = start_pc + self.functions.get(&pc).expect("Failed to lift function").0;

        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes).unwrap_or_else(|_| 
                                                             panic!("Error occured at {:#0X}", pc));
            instrs.push(instr);
            pc +=4;
        }

        if let Some(v) = self.functions.get(&start_pc) {
            if *NUM_THREADS.get().unwrap() == 1 {
                log(LogType::Neutral, &format!("Lifting: {}", v.1));
            }
        }

        // These are used to determine jump locations ahead of time
        let mut keys = self.extract_labels(start_pc, &instrs);
        keys.insert(start_pc, 0);

        self.lift(&mut irgraph, &instrs, &mut keys, start_pc);

        Ok(irgraph)
    }

    /// This function takes a set of instructions and lifts them into the intermediate
    /// representation. It uses the keys to insert labels where appropriate. These act as start
    /// markers for new code blocks.
    fn lift(&self, irgraph: &mut IRGraph, instrs: &[Instr], keys: &mut BTreeMap<usize, u8>,
            mut pc: usize) {

        // Lift instructions until we reach the end of the function
        for instr in instrs {

            irgraph.init_instr(pc);

            if keys.get(&pc).is_some() {
                irgraph.set_label(pc);
            }

            match *instr {
                Instr::Lui {rd, imm} => {
                    irgraph.movi32(rd, imm, Flag::Signed);
                },
                Instr::Auipc {rd, imm} => {
                    let result = (imm as i64 as u64).wrapping_add(pc as u64);
                    irgraph.movi64(rd, result as i64, Flag::Unsigned);
                },
                Instr::Jal {rd, imm} => {
                    let jmp_target = pc.wrapping_add(imm as i64 as usize);

                    if rd != Register::Zero {
                        irgraph.movi64(rd, pc.wrapping_add(4) as i64, Flag::Unsigned);
                    }
                    irgraph.jmp(jmp_target);
                },
                Instr::Jalr {rd, imm, rs1} => {
                    if rd != Register::Zero {
                        //irgraph.movi32(rd, (pc + 4) as i32, Flag::Signed);
                        irgraph.movi64(rd, pc.wrapping_add(4) as i64, Flag::Unsigned);
                    }
                    irgraph.jmp_offset(rs1, imm);
                },
                Instr::Beq  { rs1, rs2, imm, mode } |
                Instr::Bne  { rs1, rs2, imm, mode } |
                Instr::Blt  { rs1, rs2, imm, mode } |
                Instr::Bge  { rs1, rs2, imm, mode } |
                Instr::Bltu { rs1, rs2, imm, mode } |
                Instr::Bgeu { rs1, rs2, imm, mode } => {
                    let true_part  = pc.wrapping_add(imm as i64 as usize);
                    let false_part = pc.wrapping_add(4) as usize;

                    match mode {
                        0b000 => { /* BEQ */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                Flag::Equal | Flag::Signed)
                        },
                        0b001 => { /* BNE */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::NEqual | Flag::Signed)
                        },
                        0b100 => { /* BLT */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Less | Flag::Signed)
                        },
                        0b101 => { /* BGE */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Greater | Flag::Signed | Flag::Equal)
                        },
                        0b110 => { /* BLTU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Less | Flag::Unsigned)
                        },
                        0b111 => { /* BGEU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Greater | Flag::Unsigned | Flag::Equal)
                        },
                        _ => { unreachable!(); },
                    }
                },
                Instr::Lb  {rd, rs1, imm, mode} |
                Instr::Lh  {rd, rs1, imm, mode} |
                Instr::Lw  {rd, rs1, imm, mode} |
                Instr::Lbu {rd, rs1, imm, mode} |
                Instr::Lhu {rd, rs1, imm, mode} |
                Instr::Lwu {rd, rs1, imm, mode} |
                Instr::Ld  {rd, rs1, imm, mode} => {
                    match mode {
                        0b000 => irgraph.load(rd, rs1, imm, Flag::Byte | Flag::Signed),    // LB
                        0b001 => irgraph.load(rd, rs1, imm, Flag::Word | Flag::Signed),    // LH
                        0b010 => irgraph.load(rd, rs1, imm, Flag::DWord | Flag::Signed),   // LW
                        0b100 => irgraph.load(rd, rs1, imm, Flag::Byte | Flag::Unsigned),  // LBU
                        0b101 => irgraph.load(rd, rs1, imm, Flag::Word | Flag::Unsigned),  // LHU
                        0b110 => irgraph.load(rd, rs1, imm, Flag::DWord | Flag::Unsigned), // LWU
                        0b011 => irgraph.load(rd, rs1, imm, Flag::QWord),                  // LD
                        _ => unreachable!(),
                    };
                },
                Instr::Sb  {rs1, rs2, imm, mode} |
                Instr::Sh  {rs1, rs2, imm, mode} |
                Instr::Sw  {rs1, rs2, imm, mode} |
                Instr::Sd  {rs1, rs2, imm, mode} => {
                    match mode {
                        0b000 => { irgraph.store(rs1, rs2, imm, Flag::Byte) },  // SB
                        0b001 => { irgraph.store(rs1, rs2, imm, Flag::Word) },  // SH
                        0b010 => { irgraph.store(rs1, rs2, imm, Flag::DWord) }, // SW
                        0b011 => { irgraph.store(rs1, rs2, imm, Flag::QWord) }, // SD
                        _ => { unreachable!(); },
                    }
                },
                Instr::Addi  {rd, rs1, imm } |
                Instr::Slti  {rd, rs1, imm } |
                Instr::Sltiu {rd, rs1, imm } |
                Instr::Xori  {rd, rs1, imm } |
                Instr::Ori   {rd, rs1, imm } |
                Instr::Andi  {rd, rs1, imm } |
                Instr::Slli  {rd, rs1, imm } |
                Instr::Srli  {rd, rs1, imm } |
                Instr::Srai  {rd, rs1, imm } |
                Instr::Addiw {rd, rs1, imm } |
                Instr::Slliw {rd, rs1, imm } |
                Instr::Srliw {rd, rs1, imm } |
                Instr::Sraiw {rd, rs1, imm } => {
                    match instr {
                        Instr::Sltiu { .. } => irgraph.slti(rd, rs1, imm, Flag::Unsigned),
                        Instr::Slti  { .. } => irgraph.slti(rd, rs1, imm, Flag::Signed),
                        Instr::Addi  { .. } => irgraph.addi(rd, rs1, imm, Flag::QWord),
                        Instr::Slli  { .. } => irgraph.shli(rd, rs1, imm, Flag::QWord),
                        Instr::Srli  { .. } => irgraph.shri(rd, rs1, imm, Flag::QWord),
                        Instr::Srai  { .. } => irgraph.sari(rd, rs1, imm, Flag::QWord),
                        Instr::Addiw { .. } => irgraph.addi(rd, rs1, imm, Flag::DWord),
                        Instr::Slliw { .. } => irgraph.shli(rd, rs1, imm, Flag::DWord),
                        Instr::Srliw { .. } => irgraph.shri(rd, rs1, imm, Flag::DWord),
                        Instr::Sraiw { .. } => irgraph.sari(rd, rs1, imm, Flag::DWord),
                        Instr::Xori  { .. } => irgraph.xori(rd, rs1, imm),
                        Instr::Andi  { .. } => irgraph.andi(rd, rs1, imm),
                        Instr::Ori   { .. } => irgraph.ori(rd, rs1, imm),
                        _ => unreachable!(),
                    };
                },
                Instr::Add  {rd, rs1, rs2 } |
                Instr::Sub  {rd, rs1, rs2 } |
                Instr::Sll  {rd, rs1, rs2 } |
                Instr::Slt  {rd, rs1, rs2 } |
                Instr::Sltu {rd, rs1, rs2 } |
                Instr::Xor  {rd, rs1, rs2 } |
                Instr::Srl  {rd, rs1, rs2 } |
                Instr::Sra  {rd, rs1, rs2 } |
                Instr::Or   {rd, rs1, rs2 } |
                Instr::And  {rd, rs1, rs2 } |
                Instr::Addw {rd, rs1, rs2 } |
                Instr::Subw {rd, rs1, rs2 } |
                Instr::Sllw {rd, rs1, rs2 } |
                Instr::Srlw {rd, rs1, rs2 } |
                Instr::Sraw {rd, rs1, rs2 } => {
                    match instr   {
                        Instr::Add  { .. } => irgraph.add(rd, rs1, rs2, Flag::QWord),
                        Instr::Sub  { .. } => irgraph.sub(rd, rs1, rs2, Flag::QWord),
                        Instr::Sll  { .. } => irgraph.shl(rd, rs1, rs2, Flag::QWord),
                        Instr::Slt  { .. } => irgraph.slt(rd, rs1, rs2, Flag::Signed),
                        Instr::Sltu { .. } => irgraph.slt(rd, rs1, rs2, Flag::Unsigned),
                        Instr::Xor  { .. } => irgraph.xor(rd, rs1, rs2),
                        Instr::Srl  { .. } => irgraph.shr(rd, rs1, rs2, Flag::QWord),
                        Instr::Sra  { .. } => irgraph.sar(rd, rs1, rs2, Flag::QWord),
                        Instr::Or   { .. } => irgraph.or(rd, rs1, rs2),
                        Instr::And  { .. } => irgraph.and(rd, rs1, rs2),
                        Instr::Addw { .. } => irgraph.add(rd, rs1, rs2, Flag::DWord),
                        Instr::Subw { .. } => irgraph.sub(rd, rs1, rs2, Flag::DWord),
                        Instr::Sllw { .. } => irgraph.shl(rd, rs1, rs2, Flag::DWord),
                        Instr::Srlw { .. } => irgraph.shr(rd, rs1, rs2, Flag::DWord),
                        Instr::Sraw { .. } => irgraph.sar(rd, rs1, rs2, Flag::DWord),
                        _ => unreachable!(),
                    };
                },
                Instr::Ecall {} => {
                    irgraph.syscall();
                },
                _ => panic!("A problem occured while lifting pc={:#0X} instr={:?}", pc, instr),
            }
            pc += 4;
        }
    }
}
