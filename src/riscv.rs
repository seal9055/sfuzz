/// Trait that allows bit extractions from usizes by calling num.get_u32()
pub trait ExtractBits{
    fn get_u32(self, but_offset: u32, length: u32) -> u32;
    fn get_i32(self, but_offset: u32, length: u32) -> i32;
}
impl ExtractBits for u32 {
    fn get_u32(self, bit_offset: u32, length: u32) -> u32 {
        self << bit_offset >> 32 - length
    }
    fn get_i32(self, bit_offset: u32, length: u32) -> i32 {
        (self as i32) << bit_offset >> 32 - length
    }
}

/// Register-Register Operations
#[derive(Debug)]
pub struct RType {
    /// Type of Operation
    pub funct7: u32,

    /// Src Operand 2
    pub rs2:    u32,

    /// Src Operand 1
    pub rs1:    u32,

    /// Operation to be performed
    pub funct3: u32,

    /// Destination register
    pub rd:     u32,

    /// Operation
    pub op:     u32,
}

impl RType {
    pub fn new(instr: u32) -> Self {
        RType {
            funct7: instr.get_u32(0, 7),
            rs2:    instr.get_u32(7, 5),
            rs1:    instr.get_u32(12, 5),
            funct3: instr.get_u32(17, 3),
            rd:     instr.get_u32(20, 5),
            op:     instr.get_u32(25, 7),
        }
    }
}

/// Register-Immediate Operations (Immediate arithmetic and load)
#[derive(Debug)]
pub struct IType {
    /// Immediate constant (Integer OPS) | Offset (JALR)
    /// 2s-complement, sign extended
    pub imm:    i32,

    /// Src operand (Integer OPS) | Base (JALR)
    pub rs1:    u32,

    /// Operation to be performed
    pub funct3: u32,

    /// Destination register
    pub rd:     u32,
}

impl IType {
    pub fn new(instr: u32) -> Self {
        IType {
            imm:    instr.get_i32(0, 12),
            rs1:    instr.get_u32(12, 5),
            funct3: instr.get_u32(17, 3),
            rd:     instr.get_u32(20, 5),
        }
    }
}

/// Store instructions
#[derive(Debug)]
pub struct SType {
    /// Offset added to base address, split so rs1 and rs2 remain in constant locations
    pub imm:    i32,

    /// Source operand register
    pub rs2:    u32,

    /// Base address register
    pub rs1:    u32,

    /// Operation to be performed
    pub funct3: u32,
}

impl SType {
    pub fn new(instr: u32) -> Self {
        let imm11 = instr.get_u32(0, 7);
        let imm4  = instr.get_u32(20, 5);
        let val = (imm11 << 5) + imm4;

        SType {
            imm:    ((val as i32) << 20) >> 20,
            rs2:    instr.get_u32(7, 5),
            rs1:    instr.get_u32(12, 5),
            funct3: instr.get_u32(17, 3),
        }
    }
}

/// Used for conditional branch Instructions, compares rs1 & rs2 to determine branch
#[derive(Debug)]
pub struct BType {
    /// Offset added to base address, split so rs1 and rs2 remain in constant locations
    pub imm:    i32,

    /// Souce operand register 2
    pub rs2:    u32,

    /// Souce operand register 1
    pub rs1:    u32,

    /// Operation to be performed
    pub funct3: u32,
}

impl BType {
    pub fn new(instr: u32) -> Self {
        let imm12 = instr.get_u32(0, 1);
        let imm10 = instr.get_u32(1, 6);
        let imm4  = instr.get_u32(20, 4);
        let imm11 = instr.get_u32(24, 1);
        let val = (imm12 << 12) + (imm11 << 11) + (imm10 << 5) + (imm4 << 1);

        BType {
            imm:    ((val as i32) << 19) >> 19,
            rs2:    instr.get_u32(7, 5),
            rs1:    instr.get_u32(12, 5),
            funct3: instr.get_u32(17, 3),
        }
    }
}

/// Used to either build constants, or to construct pc-relative addresses
#[derive(Debug)]
pub struct UType {
    /// Used to build constants (LUI) or as a pc-relative address (AUIPC)
    pub imm: u32,

    /// Destination register
    pub rd:  u32,
}

impl UType {
    pub fn new(instr: u32) -> Self {
        let val = instr.get_u32(0, 20);

        UType {
            imm: (val << 12) >> 12,
            rd:  instr.get_u32(20, 5),
        }
    }
}

/// Only used by JAL instruction
#[derive(Debug)]
pub struct JType {
    /// Sign extended offset used for unconditional jump
    pub imm: i32,

    /// Address at pc+4 during the jump is stored in rd
    pub rd:  u32,
}

impl JType {
    pub fn new(instr: u32) -> Self {
        let imm20 = instr.get_u32(0, 1);
        let imm10 = instr.get_u32(1, 10);
        let imm11 = instr.get_u32(11, 1);
        let imm19 = instr.get_u32(12, 8);

        let val = (imm20 << 20) + (imm19 << 12) + (imm11 << 11) + (imm10 << 1);

        JType {
            imm: ((val as i32) << 11) >> 11,
            rd:  instr.get_u32(20, 5),
        }
    }
}
