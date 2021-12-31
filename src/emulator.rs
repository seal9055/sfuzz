use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{RType, IType, SType, BType, UType, JType},
};

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

/// Various faults that can occur during program execution. These can be syscalls, bugs, or other
/// non-standard behaviors that require kernel involvement
#[derive(Clone, Copy, Debug)]
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

#[derive(Clone)]
pub struct Emulator {
    pub memory: Mmu,

    pub state: State,
}

impl Emulator {
    pub fn new(size: usize) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
        }
    }

    pub fn set_reg(&mut self, reg: Register, val: usize) {
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    pub fn get_reg(&self, reg: Register) -> usize {
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
    }

    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        self.memory.load_segment(segment, data)
    }

    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        self.memory.allocate(size)
    }

    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        self.memory.free(addr)
    }

    pub fn run_emu(&mut self) -> Result<(), Fault> {
        loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Err(Fault::ExecFault(pc)); }

            // If an error occurs during this read, it is most likely due to missing read or execute
            // permissions, so we mark it as an ExecFault
            let instr: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc))?;

            let opcode = instr & 0b1111111;

            match opcode {
                0b0110111 => { /* LUI */
                    let _instr = UType::new(instr);

                },
                0b0010111 => { /* AUIPC */
                    let _instr = UType::new(instr);

                },
                0b1101111 => { /* JAL */
                    let _instr = JType::new(instr);

                },
                0b1100111 => { /* JALR */

                },
                0b1100011 => {
                    let instr = BType::new(instr);
                    match instr.funct3 {
                        0b000 => { /* BEQ */

                        },
                        0b001 => { /* BNE */

                        },
                        0b100 => { /* BLT */

                        },
                        0b101 => { /* BGE */

                        },
                        0b110 => { /* BLTU */

                        },
                        0b111 => { /* BGEU */

                        },
                        _ => { unreachable!(); }
                    }

                },
                0b0000011 => {
                    let instr = IType::new(instr);
                    match instr.funct3 {
                        0b000 => { /* LB */

                        },
                        0b001 => { /* LH */

                        },
                        0b010 => { /* LW */

                        },
                        0b100 => { /* LBU */

                        },
                        0b101 => { /* LHU */

                        },
                        0b110 => { /* LWU */

                        },
                        0b011 => { /* LD */

                        },
                        _ => { unreachable!(); }
                    }
                },
                0b0100011 => {
                    let instr = SType::new(instr);
                    match instr.funct3 {
                        0b000 => { /* SB */

                        },
                        0b001 => { /* SH */

                        },
                        0b010 => { /* SW */

                        },
                        0b011 => { /* SD */

                        },
                        _ => { unreachable!(); }
                    }
                },
                0b0010011 => {
                    let instr = IType::new(instr);
                    match instr.funct3 {
                        0b000 => { /* ADDI */

                        },
                        0b010 => { /* SLTI */

                        },
                        0b011 => { /* SLTIU */

                        },
                        0b100 => { /* XORI */

                        },
                        0b110 => { /* ORI */

                        },
                        0b111 => { /* ANDI */

                        },
                        0b001 => { /* SLLI */

                        },
                        0b101 => {
                            match (instr.imm >> 6) & 0b111111 {
                                0b000000 => { /* SRLI */

                                },
                                0b010000 => { /* SRAI */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b0110011 => {
                    let instr = RType::new(instr);
                    match instr.funct3 {
                        0b000 => {
                            match instr.funct7 {
                                0b0000000 => { /* ADD */

                                },
                                0b0100000 => { /* SUB */

                                },
                                0b0000001 => { /* MUL */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b001 => {
                            match instr.funct7 {
                                0b0000000 => { /* SLL */

                                },
                                0b0000001 => { /* MULH */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b010 => {
                            match instr.funct7 {
                                0b0000000 => { /* SLT */

                                },
                                0b0000001 => { /* MULHSU */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b011 => {
                            match instr.funct7 {
                                0b0000000 => { /* SLTU */

                                },
                                0b0000001 => { /* MULHU */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b100 => {
                            match instr.funct7 {
                                0b0000000 => { /* XOR */

                                },
                                0b0000001 => { /* DIV */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b101 => {
                            match instr.funct7 {
                                0b0000000 => { /* SRL */

                                },
                                0b0100000 => { /* SRA */

                                },
                                0b0000001 => { /* DIVU */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b110 => {
                            match instr.funct7 {
                                0b0000000 => { /* OR */

                                },
                                0b0000001 => { /* REM */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b111 => {
                            match instr.funct7 {
                                0b0000000 => { /* AND */

                                },
                                0b0000001 => { /* REMU */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        _ => { unreachable!(); }
                    }

                },
                0b0001111 => { /* Fence */
                    // Nop
                },
                0b1110011 => {
                    if instr == 0b00000000000000000000000001110011 { /* ECALL */
                        return Err(Fault::Syscall);
                    } else if instr == 0b00000000000100000000000001110011 { /* EBREAK */

                    } else { unreachable!(); }
                },
                0b0011011 => {
                    let instr = IType::new(instr);

                    match instr.funct3 {
                        0b000 => { /* ADDIW */

                        },
                        0b001 => { /* SLLIW */

                        },
                        0b101 => {
                            match (instr.imm >> 5) & 0b1111111 {
                                0b0000000 => { /* SRLIW */

                                },
                                0b0100000 => { /* SRAIW */

                                },
                                _ => { unreachable!(); },
                            }
                        },
                        _ => { unreachable!(); },
                    }
                }
                0b0111011 => {
                    let instr = RType::new(instr);
                    match instr.funct3 {
                        0b000 => {
                            match instr.funct7 {
                                0b0000000 => { /* ADDW */

                                },
                                0b0100000 => { /* SUBW */

                                },
                                0b0000001 => { /* MULW */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b001 => { /* SLLW */

                        },
                        0b101 => {
                            match instr.funct7 {
                                0b0000000 => { /* SRLW */

                                },
                                0b0100000 => { /* SRAW */

                                },
                                0b0000001 => { /* DIVUW */

                                },
                                _ => { unreachable!(); }
                            }
                        },
                        0b100 => { /* DIVW */

                        },
                        0b110 => { /* REMW */

                        },
                        0b111 => { /* REMUW */

                        },
                        _ => { unreachable!(); }
                    }
                },
                _ => { unreachable!(); }
            }
            self.set_reg(Register::Pc, pc.wrapping_add(4));
        }
    }
}
