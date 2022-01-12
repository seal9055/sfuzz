use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    shared::Shared,
    syscalls,
    error_exit,
};

use std::sync::Arc;
use std::collections::VecDeque;
use std::arch::asm;
use rustc_hash::FxHashMap;

use iced_x86::code_asm::*;
use iced_x86::{Formatter, Instruction, NasmFormatter};
//use rustc_hash::FxHashMap;

pub const STDIN:  isize = 0;
pub const STDOUT: isize = 1;
pub const STDERR: isize = 2;

/// 33 RISCV Registers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fault {
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    regs: [usize; 33],
}

impl Default for State {
    fn default() -> Self {
        State {
            regs: [0; 33],
        }
    }
}

// TODO remove clone, should only be arc cloned
#[derive(Clone)]
pub struct Emulator {
    pub memory: Mmu,

    pub state: State,

    pub hooks: FxHashMap<usize, fn(&mut Emulator) -> Result<(), Fault>>,

    pub shared: Arc<Shared>,

    pub fd_list: Vec<isize>,
}

impl Emulator {
    pub fn new(size: usize, shared: Arc<Shared>) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
            hooks: FxHashMap::default(),
            shared,
            fd_list: vec![STDIN, STDOUT, STDERR],
        }
    }

    pub fn set_reg(&mut self, reg: Register, val: usize) {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    pub fn jit_set_reg(&mut self, asm: &mut CodeAssembler, dst: Register, src: AsmRegister64) {
        let dst_off = self.get_reg_offset(dst);
        if dst_off != 0 { // Don't do the write if destination is zero-register
            asm.mov(ptr(r13+dst_off), src).unwrap();
        }
    }

    pub fn get_reg(&self, reg: Register) -> usize {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
    }

    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        self.memory.load_segment(segment, data)
    }

    pub fn allocate(&mut self, size: usize, perms: u8) -> Option<usize> {
        self.memory.allocate(size, perms)
    }

    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        self.memory.free(addr)
    }

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

    pub fn run_jit(&mut self) -> Option<Fault> {
        loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Some(Fault::ExecFault(pc)); }

            let jit_addr = match (*self.shared).lookup(pc) {
                None => { self.compile(pc).unwrap() }
                Some(addr) => { addr }
            };

            //println!("jit_start: {:x}", jit_addr);

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
                in("r13") self.state.regs.as_ptr() as u64,
                in("r14") self.memory.memory.as_ptr() as u64,
                in("r15") self.shared.lookup_arr.read().unwrap().as_ptr() as u64,
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
                _ => { unreachable!(); }
            }
        }
    }

    fn get_reg_offset(&self, reg: Register) -> usize {
        reg as usize * 8
    }

    /// JIT compile a function
    /// IN:
    ///     r13 points to register array in memory
    ///     r14 points to memory array
    ///     r15 points to lookup array to check if pc is jitted
    /// OUT:
    ///     rax specifies exit code
    ///         1. indirect jump, keep executing to determine jump target
    ///         2. syscall
    ///         3. hooked function reached
    ///     rcx specifies re-entry address for the jit
    fn compile(&mut self, pc: usize) -> Result<usize, IcedError> {
        let start_pc = pc;
        // This is used to remember the pc value of the previous instruction. When a hook is called,
        // this is used to determine the reentry address.
        let mut asm: CodeAssembler;
        let mut instr_queue = VecDeque::new();

        instr_queue.push_back(pc);

        while let Some(pc) = instr_queue.pop_front() {
            asm = CodeAssembler::new(64).unwrap();

            // Insert hook for addresses we want to hook with our own function and return
            if self.hooks.get(&pc).is_some() {
                asm.mov(rcx, pc as u64).unwrap();
                asm.mov(rax, 3u64).unwrap();
                asm.ret().unwrap();
                (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
                break;
            }

            // If an error occurs during this read, it is most likely due to missing read or execute
            // permissions, so we mark it as an ExecFault
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);

            //println!("0x{:x} {:?}", pc, instr);

            match instr {
                Instr::Lui {rd, imm} => {
                    asm.mov(rax, imm as i64).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Auipc {rd, imm} => {
                    //let sign_extended = (imm + pc as i32) as i64;
                    let sign_extended = (imm as i64 as u64).wrapping_add(pc as u64);
                    asm.mov(rax, sign_extended).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Jal {rd, imm} => {
                    let _pc_off = self.get_reg_offset(Register::Pc);
                    let ret_val = (pc + 4) as u64;
                    let jmp_target = (pc as i32 + imm) as usize;

                    // Jump without return so can just emit code at that location
                    if rd == Register::Zero {
                        // If the jump target has already been compiled, jump there and stop compiling
                        if let Some(addr) = self.shared.lookup(jmp_target) {
                            asm.jmp(addr as u64).unwrap();
                            (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
                            break;
                        }
                        // Otherwise since its an unconditional jump, just keep compiling
                        // The nop instruction is added to create an instruction that the Jal instr
                        // can map to upon lookup.
                        asm.nop().unwrap();
                        instr_queue.push_back(jmp_target);
                        (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
                        continue;
                    }

                    let mut jit_exit = asm.create_label();

                    // Move pc+4 into rd
                    asm.mov(rax, ret_val).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + (jmp_target * 8))).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap(); //(not in jit).unwrap();
                    asm.jmp(rax).unwrap();

                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.mov(rcx, jmp_target as u64).unwrap();
                    asm.ret().unwrap();

                    (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
                    break;
                },
                Instr::Jalr {rd, imm, rs1} => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let _pc_off = self.get_reg_offset(Register::Pc);
                    let ret_val = (pc + 4) as u64;
                    let mut jit_exit = asm.create_label();

                    // Move pc+4 into rd
                    asm.mov(rax, ret_val).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);

                    // Move jump target into rcx
                    asm.mov(rcx, ptr(r13+rs1_off)).unwrap();
                    asm.add(rcx, imm).unwrap();

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + rcx*8)).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap();
                    asm.jmp(rax).unwrap();

                    // exit jit
                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.ret().unwrap();

                    (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
                    break;
                },
                Instr::Beq  { rs1, rs2, imm, mode } |
                Instr::Bne  { rs1, rs2, imm, mode } |
                Instr::Blt  { rs1, rs2, imm, mode } |
                Instr::Bge  { rs1, rs2, imm, mode } |
                Instr::Bltu { rs1, rs2, imm, mode } |
                Instr::Bgeu { rs1, rs2, imm, mode } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    let jmp_target = pc as i32 + imm;
                    let mut jit_exit = asm.create_label();
                    let mut fallthrough = asm.create_label();

                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.cmp(rax, ptr(r13+rs2_off)).unwrap();
                    match mode {
                        0b000 => { asm.jne(fallthrough).unwrap();  }, /* BEQ  */
                        0b001 => { asm.je(fallthrough).unwrap();   }, /* BNE  */
                        0b100 => { asm.jnl(fallthrough).unwrap();  }, /* BLT  */
                        0b101 => { asm.jnge(fallthrough).unwrap(); }, /* BGE  */
                        0b110 => { asm.jnb(fallthrough).unwrap();  }, /* BLTU */
                        0b111 => { asm.jnae(fallthrough).unwrap(); }, /* BGEU */
                        _ => { unreachable!(); },
                    }

                    // Move jump target into rcx
                    asm.mov(rcx, jmp_target as u64).unwrap();

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + rcx*8)).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap();
                    asm.jmp(rax).unwrap();

                    // exit jit
                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.ret().unwrap();

                    // Fall through to next instruction
                    asm.set_label(&mut fallthrough).unwrap();

                    // Necessary because assembler otherwise struggles with labels
                    asm.nop().unwrap();
                }
                Instr::Lb  {rd, rs1, imm, mode} |
                Instr::Lh  {rd, rs1, imm, mode} |
                Instr::Lw  {rd, rs1, imm, mode} |
                Instr::Lbu {rd, rs1, imm, mode} |
                Instr::Lhu {rd, rs1, imm, mode} |
                Instr::Lwu {rd, rs1, imm, mode} |
                Instr::Ld  {rd, rs1, imm, mode} => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // Load address to retrieve memory from into rax
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.add(rax, imm).unwrap();

                    // Load bytes from memory depending on given size
                    match mode {
                        0b000 => { asm.movsx(rbx, byte_ptr(r14+rax)).unwrap(); }, /* LB  */
                        0b001 => { asm.movsx(rbx, word_ptr(r14+rax)).unwrap(); }, /* LH  */
                        0b010 => { asm.movsxd(rbx, dword_ptr(r14+rax)).unwrap();}, /* LW  */
                        0b100 => { asm.movzx(rbx, byte_ptr(r14+rax)).unwrap(); }, /* LBU */
                        0b101 => { asm.movzx(rbx, word_ptr(r14+rax)).unwrap(); }, /* LHU */
                        0b110 => { asm.movzx(ebx, dword_ptr(r14+rax)).unwrap();}, /* LWU */
                        0b011 => { asm.mov(rbx, qword_ptr(r14+rax)).unwrap();  }, /* LD  */
                        _ => { unreachable!(); },
                    }

                    // Store the result in rd
                    self.jit_set_reg(&mut asm, rd, rbx);
                },
                Instr::Sb  {rs1, rs2, imm, mode} |
                Instr::Sh  {rs1, rs2, imm, mode} |
                Instr::Sw  {rs1, rs2, imm, mode} |
                Instr::Sd  {rs1, rs2, imm, mode} => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);

                    // Get address in which memory should be stored
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // TODO remove this cause imm is a constant
                    asm.add(rax, imm).unwrap();

                    // Load bytes from memory depending on given size
                    match mode {
                        0b000 => {  /* SB  */
                            asm.mov(rbx, byte_ptr(r13+rs2_off)).unwrap();
                            asm.mov(byte_ptr(r14+rax), bl).unwrap();
                        },
                        0b001 => {  /* SH  */
                            asm.mov(rbx, word_ptr(r13+rs2_off)).unwrap();
                            asm.mov(word_ptr(r14+rax), bx).unwrap();
                        },
                        0b010 => {  /* SW  */
                            asm.mov(rbx, dword_ptr(r13+rs2_off)).unwrap();
                            asm.mov(dword_ptr(r14+rax), ebx).unwrap();
                        },
                        0b011 => {  /* SD  */
                            asm.mov(rbx, qword_ptr(r13+rs2_off)).unwrap();
                            asm.mov(qword_ptr(r14+rax), rbx).unwrap();
                        },
                        _ => { unreachable!(); },
                    }
                },
                Instr::Addi {rd, rs1, imm } => {
                    if rd == Register::Zero && rs1 == Register::Zero && imm == 0 {
                        // Nop
                    } else {
                        let rs1_off = self.get_reg_offset(rs1);
                        asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                        if imm != 0 {
                            asm.add(rax, imm).unwrap();
                        }
                        self.jit_set_reg(&mut asm, rd, rax);
                    }
                },
                Instr::Slti {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = 1 if rs1 < imm
                    asm.xor(ecx, ecx).unwrap();
                    asm.cmp(rax, imm).unwrap();
                    asm.setl(cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rcx);
                },
                Instr::Sltiu {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = 1 if rs1 < imm (unsigned)
                    asm.xor(ecx, ecx).unwrap();
                    asm.cmp(rax, imm).unwrap();
                    asm.setb(cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rcx);
                },
                Instr::Xori {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 ^ imm
                    asm.xor(rax, imm).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Ori {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 | imm
                    asm.or(rax, imm).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Andi {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 & imm
                    asm.and(rax, imm).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Slli {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 << shamt
                    asm.shl(rax, shamt).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Srli {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 >> shamt (logical)
                    asm.shr(rax, shamt).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Srai {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);

                    // rax = rs1
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();

                    // rd = rs1 >> shamt (arithmetic)
                    asm.sar(rax, shamt).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Add {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.add(rax, rbx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sub {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.sub(rax, rbx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sll {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.shl(rax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Slt {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.xor(ecx, ecx).unwrap();
                    asm.cmp(rax, rbx).unwrap();
                    asm.setl(cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rcx);
                },
                Instr::Sltu {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.xor(ecx, ecx).unwrap();
                    asm.cmp(rax, rbx).unwrap();
                    asm.setb(cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rcx);
                },
                Instr::Xor {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.xor(rax, rbx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Srl {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.shr(rax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sra {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.sar(rax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Or {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.or(rax, rbx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::And {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.and(rax, rbx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Ecall {} => {
                    asm.mov(rax, 2u64).unwrap();
                    asm.mov(rcx, (pc+4) as u64).unwrap();
                    asm.ret().unwrap();
                },
                Instr::Addiw {rd, rs1, imm } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.add(eax, imm).unwrap();
                    asm.movsxd(rax, eax).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Slliw {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.shl(eax, shamt).unwrap();
                    asm.movsxd(rax, eax).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Srliw {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.shr(eax, shamt).unwrap();
                    asm.movsxd(rax, eax).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sraiw {rd, rs1, shamt } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.sar(eax, shamt).unwrap();
                    asm.movsxd(rax, eax).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Addw {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.add(eax, ebx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Subw {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(r13+rs2_off)).unwrap();
                    asm.sub(eax, ebx).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sllw {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.shl(eax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Srlw {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.shr(eax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                Instr::Sraw {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    asm.mov(rax, ptr(r13+rs1_off)).unwrap();
                    asm.mov(rcx, ptr(r13+rs2_off)).unwrap();
                    asm.sar(eax, cl).unwrap();
                    self.jit_set_reg(&mut asm, rd, rax);
                },
                _ => {
                    self.dump_instrs(asm.instructions(), start_pc, pc);
                    panic!("unimplemented instruction \
                           hit\npc: 0x{:x} \nopcodes: {:x} \ninstr: {:?}", pc, opcodes, instr);
                },
            }
            (*self.shared).add_jitblock(&asm.assemble(0x0).unwrap(), pc);
            instr_queue.push_back(pc + 4);
        }
        //println!("Start pc is: 0x{:x}", start_pc);

        Ok(self.shared.lookup(start_pc).unwrap())
    }

    fn dump_instrs(&self, instrs: &[Instruction], mut pc: usize, end_pc: usize) {
        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                    Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);
            println!("0x{:x}: {:?}", pc, instr);
            pc+=4;
        }

        for instr in instrs {
            let mut formatter = NasmFormatter::new();
            let mut output = String::new();

            output.clear();
            formatter.format(&instr, &mut output);
            println!("{:#?}", output);
        }
    }
}


/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary() {
        let shared = Arc::new(Shared::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(1024 * 1024, shared);

        let addr = emu.allocate(0x40, Perms::READ | Perms::WRITE | Perms::EXECUTE).unwrap();
        emu.set_reg(Register::Pc, addr);

        let data = std::fs::read("tests/output").unwrap();
        emu.memory.write_mem(addr, &data, data.len()).unwrap();

        println!("size: {}", data.len());
        emu.run_jit();
    }
}
*/
