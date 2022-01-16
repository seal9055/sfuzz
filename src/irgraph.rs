use rustc_hash::FxHashMap;

/// Errors that can occur during IR Operations
#[derive(Debug)]
pub enum Error {
    /// Ran out of registers for graph
    OutOfRegs,

    /// Ran out of labels for graph
    OutOfLabels,
}

/// Register-type used internally by the IR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reg(pub u16);

/// A label used for control flow in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(pub u16);

#[derive(Debug, Copy, Clone)]
enum Operation {
    Loadi(u32),
    Jmp(usize),
    Branch(usize),
    Syscall,
    JmpReg,
    Store,
    Load,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Slt,
}

#[derive(Debug, Clone, Copy)]
pub struct Flag;
#[allow(non_upper_case_globals)]
impl Flag {
    pub const NoFlag:   u16 = 0x0;
    pub const Signed:   u16 = 0x1;
    pub const Unsigned: u16 = 0x2;
    pub const Equal:    u16 = 0x4;
    pub const NEqual:   u16 = 0x8;
    pub const Less:     u16 = 0x10;
    pub const Greater:  u16 = 0x20;
    pub const Byte:     u16 = 0x40;
    pub const Word:     u16 = 0x80;
    pub const DWord:    u16 = 0x100;
    pub const QWord:    u16 = 0x200;
}

#[derive(Debug, Copy, Clone)]
pub struct Instruction {
    op:    Operation,
    i_reg:  (Option<Reg>, Option<Reg>),
    o_reg:  Option<Reg>,
    flags: u16,
    pc:    Option<usize>,
}

#[derive(Debug)]
pub struct IRGraph {
    /// List of all instructions
    pub instrs: Vec<Instruction>,

    /// Currently available register index
    next_reg: Reg,

    /// Named labels used for control flow
    /// Maps pc to index in instr array
    labels: FxHashMap<usize, u16>,

    /// This is used to map track the pc of the currently executing instruction
    cur_pc: Option<usize>,

    // Track which functions have already been jitted, and their adresses, prob in the jit though
}

impl Default for IRGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl IRGraph {
    pub fn new() -> Self {
        IRGraph {
            instrs:     Vec::new(),
            next_reg:   Reg(0),
            labels:     FxHashMap::default(),
            cur_pc:     None,
        }
    }

    /// Optimize the IRGraph
    pub fn optimize(&mut self) -> Option<()> {
        // Probably convert to cfg notation before starting optimizations
        // Need to research different types of optimizations a little first, the main optimization
        // will be to improve codegen involving immediate instructions because these currently take
        // way too many instructions
        Some(())
    }

    /// Initialize the cur_pc variable which is used to set the pc value in the ir instructions
    pub fn init_instr(&mut self, pc: usize) {
        self.cur_pc = Some(pc);
    }

    /// Allocate new Register for IRGraph
    fn alloc_reg(&mut self) -> Result<Reg, Error> {
        let ret = self.next_reg;
        self.next_reg = Reg(self.next_reg.0.checked_add(1).ok_or(Error::OutOfRegs)?);
        Ok(ret)
    }

    /// Allocate new Label for IRGraph
    pub fn set_label(&mut self) {
        self.labels.insert(self.cur_pc.unwrap(), self.instrs.len() as u16);
    }

    /// Load an immediate value into a register
    pub fn loadi(&mut self, imm: u32, flag: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Loadi(imm),
            i_reg: (None, None),
            o_reg: Some(reg),
            flags: flag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn jmp(&mut self, target: usize) {
        self.instrs.push( Instruction {
            op: Operation::Jmp(target),
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn jmp_reg(&mut self, target: Reg) {
        self.instrs.push( Instruction {
            op: Operation::JmpReg,
            i_reg: (Some(target), None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn branch(&mut self, reg1: Reg, reg2: Reg, imm: usize, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Branch(imm),
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn load(&mut self, reg1: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Load,
            i_reg: (Some(reg1), None),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn store(&mut self, rs2_reg: Reg, mem_addr: Reg, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Store,
            i_reg: (Some(rs2_reg), Some(mem_addr)),
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Set res_reg if rs1_reg is less than imm_reg
    pub fn slt(&mut self, rs1_reg: Reg, imm_reg: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Slt,
            i_reg: (Some(rs1_reg), Some(imm_reg)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    /// Add 2 registers and store the result in a new register
    pub fn add(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Add,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    /// Subtract ret2 from reg1 and store the result in a new register
    pub fn sub(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Sub,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn xor(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Xor,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn or(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Or,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn and(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::And,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn shl(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Shl,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn shr(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Shr,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn sar(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Sar,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn syscall(&mut self) {
         self.instrs.push( Instruction {
            op: Operation::Syscall,
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }
}
