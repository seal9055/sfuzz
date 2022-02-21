use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    jit::Jit,
    syscalls,
    error_exit,
    irgraph::{IRGraph, Flag},
    ssa_builder::SSABuilder,
    regalloc::Regalloc,
};

use std::sync::Arc;
use std::collections::BTreeMap;
use std::arch::asm;
use rustc_hash::FxHashMap;

/// File Descriptors
pub const STDIN:  isize = 0;
pub const STDOUT: isize = 1;
pub const STDERR: isize = 2;

/// Number of registers (33 Riscv Regs + 2 temporary regs needed for ir-gen)
pub const NUMREGS: usize = 35;

/// 33 RISCV Registers + 2 Extra temporary registers that the IR needs
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
    Z1, // Z[1-2] are temporary registers used to generate ir instructions
    Z2,
}

impl From<u32> for Register {
    /// Convert a number to a Register enum
    fn from(val: u32) -> Self {
        assert!(val < NUMREGS as u32);
        unsafe {
            core::ptr::read_unaligned(&(val as usize) as *const usize as *const Register)
        }
    }
}

/// Various faults that can occur during program execution. These can be syscalls, bugs, or other
/// non-standard behaviors that require kernel involvement
#[derive(Clone, Copy, Debug, PartialEq)] pub enum Fault {
    /// Syscall
    Syscall,

    /// Fault occurs when an attempt is made to write to an address without Perms::WRITE set
    WriteFault(usize),

    /// Fault occurs when an attempt is made to read from an address without Perms::READ set
    ReadFault(usize),

    /// Fault occurs when an attempt is made to execute an invalid instruction
    ExecFault(usize),

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
}

/// Describes the current state of the emulator
#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub regs: [usize; 33],
}

impl Default for State {
    fn default() -> Self {
        State {
            regs: [0; 33],
        }
    }
}

/// Emulator that runs the actual code. Each thread gets its own emulator in which everything is
/// separate except the jit backing that all emulators share.
#[derive(Clone)]
pub struct Emulator {
    /// Memory backing for the emulator, contains actual memory bytes and permissions
    pub memory: Mmu,

    /// Describes the current state of the emulator
    pub state: State,

    /// These are used to hook specific addresses. Can be used for debug purposes or to redirect
    /// important functions such as malloc/free to our own custom implementations
    pub hooks: FxHashMap<usize, fn(&mut Emulator) -> Result<(), Fault>>,

    /// The actual jit compiler backing
    pub jit: Arc<Jit>,

    /// List of file descriptors that the process can use for syscalls
    pub fd_list: Vec<isize>,

    /// Mapping between function address and function size
    pub functions: FxHashMap<usize, usize>,
}

impl Emulator {
    pub fn new(size: usize, jit: Arc<Jit>) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
            hooks: FxHashMap::default(),
            jit,
            fd_list: vec![STDIN, STDOUT, STDERR],
            functions: FxHashMap::default(),
        }
    }

    /// Fork an emulator into a new one, basically creating an exact copy
    #[must_use]
    pub fn fork(&self) -> Self {
        Emulator {
            memory:     self.memory.fork(),
            state:      State::default(),
            hooks:      self.hooks.clone(),
            jit:        self.jit.clone(),
            fd_list:    self.fd_list.clone(),
            functions:  self.functions.clone(),
        }
    }

    /// Reset the entire state of this emulator (memory & registers)
    pub fn reset(&mut self, original: &Self) {
        self.memory.reset(&original.memory);
        self.state.regs = original.state.regs;

        self.fd_list.clear();
        self.fd_list.extend_from_slice(&original.fd_list);
    }

    /// Set a register
    pub fn set_reg(&mut self, reg: Register, val: usize) {
        assert!((reg as usize) < NUMREGS);
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    /// Get the value stored in a register
    pub fn get_reg(&self, reg: Register) -> usize {
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
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

    /// Debug print for registers
    pub fn dump_regs(&self) {
        println!("zero {:x?}", self.get_reg(Register::Zero));
        println!("ra   {:x?}", self.get_reg(Register::Ra));
        println!("sp   {:x?}", self.get_reg(Register::Sp));
        println!("gp   {:x?}", self.get_reg(Register::Gp));
        println!("tp   {:x?}", self.get_reg(Register::Tp));
        println!("t0   {:x?}", self.get_reg(Register::T0));
        println!("t1   {:x?}", self.get_reg(Register::T1));
        println!("t2   {:x?}", self.get_reg(Register::T2));
        println!("s0   {:x?}", self.get_reg(Register::S0));
        println!("s1   {:x?}", self.get_reg(Register::S1));
        println!("a0   {:x?}", self.get_reg(Register::A0));
        println!("a1   {:x?}", self.get_reg(Register::A1));
        println!("a2   {:x?}", self.get_reg(Register::A2));
        println!("a3   {:x?}", self.get_reg(Register::A3));
        println!("a4   {:x?}", self.get_reg(Register::A4));
        println!("a5   {:x?}", self.get_reg(Register::A5));
        println!("a6   {:x?}", self.get_reg(Register::A6));
        println!("a7   {:x?}", self.get_reg(Register::A7));
        println!("s2   {:x?}", self.get_reg(Register::S2));
        println!("s3   {:x?}", self.get_reg(Register::S3));
        println!("s4   {:x?}", self.get_reg(Register::S4));
        println!("s5   {:x?}", self.get_reg(Register::S5));
        println!("s6   {:x?}", self.get_reg(Register::S6));
        println!("s7   {:x?}", self.get_reg(Register::S7));
        println!("s8   {:x?}", self.get_reg(Register::S8));
        println!("s9   {:x?}", self.get_reg(Register::S9));
        println!("s10  {:x?}", self.get_reg(Register::S10));
        println!("s11  {:x?}", self.get_reg(Register::S11));
        println!("t3   {:x?}", self.get_reg(Register::T3));
        println!("t4   {:x?}", self.get_reg(Register::T4));
        println!("t5   {:x?}", self.get_reg(Register::T5));
        println!("t6   {:x?}", self.get_reg(Register::T6));
        println!("pc   {:x?}", self.get_reg(Register::Pc));
    }

    /// Runs the jit until exit/crash. It checks if the code at `pc` has already been compiled. If
    /// not it starts by initiating the procedure to compile the code. At this point it has the
    /// jitcache address of where `pc` is jit compiled too. Next it sets up various arguments and
    /// calls this code. These arguments point to structures in memory that the jit-code needs to
    /// convert original addresses to their corresponding jit addresses.
    /// Once the jit exits it collects the reentry_pc (where to continue execution), and the exit
    /// code. It performs an appropriate operation based on the exit code and then continues with
    /// the loop to reenter the jit.
    pub fn run_jit(&mut self, pointers: &[u64]) -> Option<Fault> {
        loop {
            let pc     = self.get_reg(Register::Pc);
            let end_pc = pc + self.functions.get(&pc).unwrap();

            println!("pc is: 0x{:x}", pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Some(Fault::ExecFault(pc)); }

            let jit_addr = match (*self.jit).lookup(pc) {
                None => {

                    let irgraph = self.lift_func(pc).unwrap();

                    let mut ssa = SSABuilder::new(&irgraph, end_pc);
                    ssa.build_ssa();

                    ssa.destruct();
                    ssa.dump_dot();

                    let mut reg_allocator = Regalloc::new(&ssa);
                    let reg_mapping = reg_allocator.get_reg_mapping();

                    let labels: Vec<usize> = irgraph.labels.iter().map(|e| *e.0).collect();
                    self.jit.compile(&ssa, &reg_mapping, labels).unwrap()
                }
                Some(addr) => { addr }
            };

            let exit_code:  usize;
            let reentry_pc: usize;
            unsafe {
                let func = *(&jit_addr as *const usize as *const fn());

                asm!(r#"
                    call {call_dest}
                "#,
                call_dest = in(reg) func,
                out("rax") exit_code,
                out("rcx") reentry_pc,
                out("rdx") _,
                in("r15") pointers.as_ptr() as u64,
                );
            }

            self.set_reg(Register::Pc, reentry_pc);

            match exit_code {
                1 => { /* Nothing special, just need to compile next code block */
                },
                2 => { /* SYSCALL */
                    match self.get_reg(Register::A7) {
                        57 => {
                            syscalls::close(self);
                        },
                        64 => {
                            syscalls::write(self);
                        },
                        80 => {
                            syscalls::fstat(self);
                        },
                        93 => {
                            return syscalls::exit();
                        },
                        214 => {
                            syscalls::brk(self);
                        },
                        _ => { panic!("Unimplemented syscall: {}", self.get_reg(Register::A7)); }
                    }
                },
                3 => { /* Hooked function */
                    if let Some(callback) = self.hooks.get(&reentry_pc) {
                        callback(self).unwrap();
                    } else {
                        error_exit("Attempted to hook invalid function");
                    }
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
    fn lift_func(&mut self, mut pc: usize) -> Result<IRGraph, ()> {
        let mut irgraph = IRGraph::default();
        let mut instrs: Vec<Instr> = Vec::new();

        let start_pc = pc;
        let end_pc   = start_pc + self.functions.get(&pc).unwrap();

        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);
            instrs.push(instr);
            pc +=4;
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
    fn lift(&mut self, irgraph: &mut IRGraph, instrs: &[Instr], keys: &mut BTreeMap<usize, u8>,
            mut pc: usize) {

        // Lift instructions until we reach the end of the function
        for instr in instrs {

            irgraph.init_instr(pc);

            if keys.get(&pc).is_some() {
                irgraph.set_label(pc);
            }

            match *instr {
                Instr::Lui {rd, imm} => {
                    irgraph.loadi(rd, imm, Flag::Signed);
                },
                Instr::Auipc {rd, imm} => {
                    let val = (imm).wrapping_add(pc as i32);
                    irgraph.loadi(rd, val, Flag::Signed);
                },
                Instr::Jal {rd, imm} => {
                    let ret_val = pc.wrapping_add(4);
                    let jmp_target = ((pc as i32).wrapping_add(imm)) as usize;

                    // Load return value into newly allocated register
                    if rd != Register::Zero {
                        irgraph.loadi(rd, ret_val as i32, Flag::Unsigned);
                        irgraph.call(jmp_target);
                    } else {
                        irgraph.jmp(jmp_target);
                    }
                },
                Instr::Jalr {rd, imm, rs1} => {
                    if rd != Register::Zero {
                        let ret_val = pc.wrapping_add(4);
                        let imm_reg = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                        let jmp_target = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                        irgraph.loadi(rd, ret_val as i32, Flag::Unsigned);
                        irgraph.call_reg(jmp_target)
                    } else {
                        irgraph.ret();
                    }
                },
                Instr::Beq  { rs1, rs2, imm, mode } |
                Instr::Bne  { rs1, rs2, imm, mode } |
                Instr::Blt  { rs1, rs2, imm, mode } |
                Instr::Bge  { rs1, rs2, imm, mode } |
                Instr::Bltu { rs1, rs2, imm, mode } |
                Instr::Bgeu { rs1, rs2, imm, mode } => {
                    let true_part  = ((pc as i32).wrapping_add(imm)) as usize;
                    let false_part = ((pc as i32).wrapping_add(4)) as usize;

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
                    let imm_reg  = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let mem_addr = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                    match mode {
                        0b000 => irgraph.load(rd, mem_addr, Flag::Byte | Flag::Signed),    // LB
                        0b001 => irgraph.load(rd, mem_addr, Flag::Word | Flag::Signed),    // LH
                        0b010 => irgraph.load(rd, mem_addr, Flag::DWord | Flag::Signed),   // LW
                        0b100 => irgraph.load(rd, mem_addr, Flag::Byte | Flag::Unsigned),  // LBU
                        0b101 => irgraph.load(rd, mem_addr, Flag::Word | Flag::Unsigned),  // LHU
                        0b110 => irgraph.load(rd, mem_addr, Flag::DWord | Flag::Unsigned), // LWU
                        0b011 => irgraph.load(rd, mem_addr, Flag::QWord),                  // LD
                        _ => unreachable!(),
                    };
                },
                Instr::Sb  {rs1, rs2, imm, mode} |
                Instr::Sh  {rs1, rs2, imm, mode} |
                Instr::Sw  {rs1, rs2, imm, mode} |
                Instr::Sd  {rs1, rs2, imm, mode} => {
                    let imm_reg  = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let mem_addr = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                    match mode {
                        0b000 => { irgraph.store(rs2, mem_addr, Flag::Byte) },  // SB
                        0b001 => { irgraph.store(rs2, mem_addr, Flag::Word) },  // SH
                        0b010 => { irgraph.store(rs2, mem_addr, Flag::DWord) }, // SW
                        0b011 => { irgraph.store(rs2, mem_addr, Flag::QWord) }, // SD
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
                    let imm_reg = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    match instr {
                        Instr::Addi  { .. } => irgraph.add(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Slti  { .. } => irgraph.slt(rd, rs1, imm_reg, Flag::Signed),
                        Instr::Sltiu { .. } => irgraph.slt(rd, rs1, imm_reg, Flag::Unsigned),
                        Instr::Xori  { .. } => irgraph.xor(rd, rs1, imm_reg),
                        Instr::Ori   { .. } => irgraph.or(rd, rs1, imm_reg),
                        Instr::Andi  { .. } => irgraph.and(rd, rs1, imm_reg),
                        Instr::Slli  { .. } => irgraph.shl(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Srli  { .. } => irgraph.shr(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Srai  { .. } => irgraph.sar(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Addiw { .. } => irgraph.add(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Slliw { .. } => irgraph.shl(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Srliw { .. } => irgraph.shr(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Sraiw { .. } => irgraph.sar(rd, rs1, imm_reg, Flag::DWord),
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
                _ => { panic!("A problem occured while lifting to IR"); },
            }
            pc += 4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_test() {
        let mut instrs: Vec<Instr> = Vec::new();
        let jit = Arc::new(Jit::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(64 * 1024 * 1024, jit);

        /*0x1000*/ instrs.push(Instr::Lui { rd: Register::A0, imm: 20 });
        /*0x1004*/ instrs.push(Instr::Lui { rd: Register::A1, imm: 10 });
        /*0x1008*/ instrs.push(Instr::Beq { rs1: Register::A0, rs2: Register::A1,
            imm: 0x20, mode: 0});

        //b1
        /*0x100c*/ instrs.push(Instr::Add { rd: Register::A2, rs1: Register::A0,
            rs2: Register::A1 });
        /*0x1010*/ instrs.push(Instr::Lui { rd: Register::A3, imm: 1 });
        /*0x1014*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: 0x4 }); //goto b2

        //b2
        /*0x1018*/ instrs.push(Instr::Addi { rd: Register::A4, rs1: Register::A2, imm: 5});
        /*0x101c*/ instrs.push(Instr::Addi { rd: Register::A5, rs1: Register::A4, imm: 1});
        /*0x1020*/ instrs.push(Instr::Addi { rd: Register::A6, rs1: Register::A3, imm: 0 });
        /*0x1024*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: 0x10 }); //goto end

        //b3
        /*0x1028*/ instrs.push(Instr::Sub { rd: Register::A2, rs1: Register::A0,
            rs2: Register::A1 });
        /*0x102c*/ instrs.push(Instr::Lui { rd: Register::A3, imm: 2 });
        /*0x1030*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: -0x18 }); //goto b2

        /*0x1034*/ instrs.push(Instr::Lui { rd: Register::Zero, imm: 0 });
        /*0x101c*/ instrs.push(Instr::Jalr { rd: Register::Zero, imm: 0, rs1: Register::A0 });

        // end


        let mut keys: Vec<usize> = emu.extract_labels(0x1000, &instrs).keys().cloned().collect();
        keys.insert(0, 0x1000);

        let mut irgraph = IRGraph::default();
        emu.lift(&mut irgraph, &instrs, &mut keys, 0x1000);

        let mut ssa = SSABuilder::new(&irgraph);
        ssa.build_ssa();

        ssa.destruct();
        ssa.dump_dot();

        let mut reg_allocator = Regalloc::new(&ssa);
        let reg_mapping = reg_allocator.get_reg_mapping();

        let labels: Vec<usize> = irgraph.labels.iter().map(|e| *e.0).collect();
        emu.jit.compile(&ssa, &reg_mapping, labels);

        emu.set_reg(Register::Pc, 0x1000);
        emu.run_jit();
    }

    #[test]
    fn loop_test() {
        let mut instrs: Vec<Instr> = Vec::new();
        let jit = Arc::new(Jit::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(64 * 1024 * 1024, jit);

        // .b1
        /*0x1000*/ instrs.push(Instr::Lui { rd: Register::A0, imm: 0x3 });
        /*0x1004*/ instrs.push(Instr::Lui { rd: Register::A1, imm: 0x0 });
        /*0x1008*/ instrs.push(Instr::Lui { rd: Register::A2, imm: 0x9 });
        /*0x100c*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: 0xc }); // jmp .b3

        // .b2
        /*0x1010*/ instrs.push(Instr::Addi { rd: Register::A0, rs1: Register::A0, imm: 0x3 });
        /*0x1014*/ instrs.push(Instr::Addi { rd: Register::A1, rs1: Register::A0, imm: 0x1 });

        // .b3
        /*0x1018*/ instrs.push(Instr::Bne { rs1: Register::A1, rs2: Register::A2,  // branch to .b2
            imm: -0x8, mode: 1});

        // .end
        /*0x101c*/ instrs.push(Instr::Jalr { rd: Register::Zero, imm: 0, rs1: Register::A0 });

        let mut keys: Vec<usize> = emu.extract_labels(0x1000, &instrs).keys().cloned().collect();
        keys.insert(0, 0x1000);

        let mut irgraph = IRGraph::default();
        emu.lift(&mut irgraph, &instrs, &mut keys, 0x1000);

        println!("GRAPH: {:?}", irgraph);

        let mut ssa_builder = SSABuilder::new(&irgraph);
        ssa_builder.build_ssa();
        ssa_builder.dump_dot();
    }
}
