use crate::emulator::Register;

#[derive(Clone, Copy, Debug)]
pub enum Instr {
    Undefined,
    Ecall,
    Ebreak,
    Add    { rd: Register, rs1: Register, rs2: Register },
    Sub    { rd: Register, rs1: Register, rs2: Register },
    Mul    { rd: Register, rs1: Register, rs2: Register },
    Mulh   { rd: Register, rs1: Register, rs2: Register },
    Div    { rd: Register, rs1: Register, rs2: Register },
    Addi   { rd: Register, rs1: Register, imm: i32 },
    Slt    { rd: Register, rs1: Register, rs2: Register },
    Slti   { rd: Register, rs1: Register, imm: i32 },
    Sltu   { rd: Register, rs1: Register, rs2: Register },
    Sltiu  { rd: Register, rs1: Register, imm: i32 },
    Lui    { rd: Register, imm: i32 },
    Auipc  { rd: Register, imm: i32 },
    And    { rd: Register, rs1: Register, rs2: Register },
    Or     { rd: Register, rs1: Register, rs2: Register },
    Xor    { rd: Register, rs1: Register, rs2: Register },
    Andi   { rd: Register, rs1: Register, imm: i32 },
    Ori    { rd: Register, rs1: Register, imm: i32 },
    Xori   { rd: Register, rs1: Register, imm: i32 },
    Sll    { rd: Register, rs1: Register, rs2: Register },
    Srl    { rd: Register, rs1: Register, rs2: Register },
    Sra    { rd: Register, rs1: Register, rs2: Register },
    Slli   { rd: Register, rs1: Register, shamt: i32 },
    Srli   { rd: Register, rs1: Register, shamt: i32 },
    Srai   { rd: Register, rs1: Register, shamt: i32 },
    Ld     { rd: Register, rs1: Register, imm: i32 },
    Lw     { rd: Register, rs1: Register, imm: i32 },
    Lh     { rd: Register, rs1: Register, imm: i32 },
    Lb     { rd: Register, rs1: Register, imm: i32 },
    Lwu    { rd: Register, rs1: Register, imm: i32 },
    Lbu    { rd: Register, rs1: Register, imm: i32 },
    Lhu    { rd: Register, rs1: Register, imm: i32 },
    Sd     { rs1: Register, rs2: Register, imm: i32 },
    Sw     { rs1: Register, rs2: Register, imm: i32 },
    Sh     { rs1: Register, rs2: Register, imm: i32 },
    Sb     { rs1: Register, rs2: Register, imm: i32 },
    Beq    { rs1: Register, rs2: Register, imm: i32 },
    Bne    { rs1: Register, rs2: Register, imm: i32 },
    Bge    { rs1: Register, rs2: Register, imm: i32 },
    Bgeu   { rs1: Register, rs2: Register, imm: i32 },
    Blt    { rs1: Register, rs2: Register, imm: i32 },
    Bltu   { rs1: Register, rs2: Register, imm: i32 },
    Jal    { rd: Register, imm: i32 },
    Jalr   { rd: Register, rs1: Register, imm: i32 },
    Mulhsu { rd: Register, rs1: Register, rs2: Register },
    Mulhu  { rd: Register, rs1: Register, rs2: Register },
    Divu   { rd: Register, rs1: Register, rs2: Register },
    Rem    { rd: Register, rs1: Register, rs2: Register },
    Remu   { rd: Register, rs1: Register, rs2: Register },
    Addiw  { rd: Register, rs1: Register, imm: i32 },
    Slliw  { rd: Register, rs1: Register, shamt: i32 },
    Srliw  { rd: Register, rs1: Register, shamt: i32 },
    Sraiw  { rd: Register, rs1: Register, shamt: i32 },
    Addw   { rd: Register, rs1: Register, rs2: Register },
    Subw   { rd: Register, rs1: Register, rs2: Register },
    Mulw   { rd: Register, rs1: Register, rs2: Register },
    Divuw  { rd: Register, rs1: Register, rs2: Register },
    Sllw  { rd: Register, rs1: Register, rs2: Register },
    Srlw  { rd: Register, rs1: Register, rs2: Register },
    Sraw  { rd: Register, rs1: Register, rs2: Register },
    Divw  { rd: Register, rs1: Register, rs2: Register },
    Remw  { rd: Register, rs1: Register, rs2: Register },
    Remuw  { rd: Register, rs1: Register, rs2: Register },
}

/// Trait that allows bit extractions from usizes by calling num.get_u32()
pub trait ExtractBits{
    fn get_u32(self, but_offset: u32, length: u32) -> u32;
    fn get_i32(self, but_offset: u32, length: u32) -> i32;
}

impl ExtractBits for u32 {
    fn get_u32(self, bit_offset: u32, length: u32) -> u32 {
        self << bit_offset >> (32 - length)
    }
    fn get_i32(self, bit_offset: u32, length: u32) -> i32 {
        (self as i32) << bit_offset >> (32 - length)
    }
}

/// Register-Register Operations
#[derive(Debug)]
pub struct RType {
    /// Type of Operation
    pub funct7: u32,

    /// Src Operand 2
    pub rs2:    Register,

    /// Src Operand 1
    pub rs1:    Register,

    /// Operation to be performed
    pub funct3: u32,

    /// Destination register
    pub rd:     Register,
}

impl RType {
    pub fn new(instr: u32) -> Self {
        RType {
            funct7: instr.get_u32(0, 7),
            rs2:    Register::from(instr.get_u32(7, 5)),
            rs1:    Register::from(instr.get_u32(12, 5)),
            funct3: instr.get_u32(17, 3),
            rd:     Register::from(instr.get_u32(20, 5)),
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
    pub rs1:    Register,

    /// Operation to be performed
    pub funct3: u32,

    /// Destination register
    pub rd:     Register,
}

impl IType {
    pub fn new(instr: u32) -> Self {
        IType {
            imm:    instr.get_i32(0, 12),
            rs1:    Register::from(instr.get_u32(12, 5)),
            funct3: instr.get_u32(17, 3),
            rd:     Register::from(instr.get_u32(20, 5)),
        }
    }
}

/// Store instructions
#[derive(Debug)]
pub struct SType {
    /// Offset added to base address, split so rs1 and rs2 remain in constant locations
    pub imm:    i32,

    /// Source operand register
    pub rs2:    Register,

    /// Base address register
    pub rs1:    Register,

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
            rs2:    Register::from(instr.get_u32(7, 5)),
            rs1:    Register::from(instr.get_u32(12, 5)),
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
    pub rs2:    Register,

    /// Souce operand register 1
    pub rs1:    Register,

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
            rs2:    Register::from(instr.get_u32(7, 5)),
            rs1:    Register::from(instr.get_u32(12, 5)),
            funct3: instr.get_u32(17, 3),
        }
    }
}

/// Used to either build constants, or to construct pc-relative addresses
#[derive(Debug)]
pub struct UType {
    /// Used to build constants (LUI) or as a pc-relative address (AUIPC)
    pub imm: i32,

    /// Destination register
    pub rd:  Register,
}

impl UType {
    pub fn new(instr: u32) -> Self {
        let val = instr.get_u32(0, 20);

        UType {
            imm: ((val << 12) >> 12) as i32,
            rd:  Register::from(instr.get_u32(20, 5)),
        }
    }
}

/// Only used by JAL instruction
#[derive(Debug)]
pub struct JType {
    /// Sign extended offset used for unconditional jump
    pub imm: i32,

    /// Address at pc+4 during the jump is stored in rd
    pub rd:  Register,
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
            rd:  Register::from(instr.get_u32(20, 5)),
        }
    }
}

pub fn decode_instr(instr: u32) -> Instr {
    let opcode = instr & 0b1111111;

    match opcode {
        0b0110111 => { /* LUI */
            let instr = UType::new(instr);
            return Instr::Lui { rd: instr.rd, imm: instr.imm };
        },
        0b0010111 => { /* AUIPC */
            let instr = UType::new(instr);
            return Instr::Auipc { rd: instr.rd, imm: instr.imm };

        },
        0b1101111 => { /* JAL */
            let instr = JType::new(instr);
            return Instr::Jal { rd: instr.rd, imm: instr.imm };

        },
        0b1100111 => { /* JALR */
            let instr = IType::new(instr);
            return Instr::Jalr { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };

        },
        0b1100011 => {
            let instr = BType::new(instr);
            match instr.funct3 {
                0b000 => { /* BEQ */
                    return Instr::Beq { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b001 => { /* BNE */
                    return Instr::Bne { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b100 => { /* BLT */
                    return Instr::Blt { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b101 => { /* BGE */
                    return Instr::Bge { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b110 => { /* BLTU */
                    return Instr::Bltu { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b111 => { /* BGEU */
                    return Instr::Bgeu { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                _ => { unreachable!(); }
            }

        },
        0b0000011 => {
            let instr = IType::new(instr);
            match instr.funct3 {
                0b000 => { /* LB */
                    return Instr::Lb { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b001 => { /* LH */
                    return Instr::Lh { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b010 => { /* LW */
                    return Instr::Lw { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b100 => { /* LBU */
                    return Instr::Lbu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b101 => { /* LHU */
                    return Instr::Lhu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b110 => { /* LWU */
                    return Instr::Lwu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b011 => { /* LD */
                    return Instr::Ld { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                _ => { unreachable!(); }
            }
        },
        0b0100011 => {
            let instr = SType::new(instr);
            match instr.funct3 {
                0b000 => { /* SB */
                    return Instr::Sb { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b001 => { /* SH */
                    return Instr::Sh { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b010 => { /* SW */
                    return Instr::Sw { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                0b011 => { /* SD */
                    return Instr::Sd { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm };
                },
                _ => { unreachable!(); }
            }
        },
        0b0010011 => {
            let instr = IType::new(instr);
            match instr.funct3 {
                0b000 => { /* ADDI */
                    return Instr::Addi { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b010 => { /* SLTI */
                    return Instr::Slti { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b011 => { /* SLTIU */
                    return Instr::Sltiu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b100 => { /* XORI */
                    return Instr::Xori { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b110 => { /* ORI */
                    return Instr::Ori { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b111 => { /* ANDI */
                    return Instr::Andi { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b001 => { /* SLLI */
                    let this_shamt = instr.imm & 0b111111;
                    return Instr::Slli { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt};
                },
                0b101 => {
                    match (instr.imm >> 6) & 0b111111 {
                        0b000000 => { /* SRLI */
                            let this_shamt = instr.imm & 0b111111;
                            return Instr::Srli { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt };
                        },
                        0b010000 => { /* SRAI */
                            let this_shamt = instr.imm & 0b111111;
                            return Instr::Srai { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt };
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
                           return Instr::Add { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0100000 => { /* SUB */
                           return Instr::Sub { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* MUL */
                           return Instr::Mul { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b001 => {
                    match instr.funct7 {
                        0b0000000 => { /* SLL */
                           return Instr::Sll { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };

                        },
                        0b0000001 => { /* MULH */
                           return Instr::Mulh { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b010 => {
                    match instr.funct7 {
                        0b0000000 => { /* SLT */
                           return Instr::Slt { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* MULHSU */
                           return Instr::Mulhsu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b011 => {
                    match instr.funct7 {
                        0b0000000 => { /* SLTU */
                           return Instr::Sltu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };

                        },
                        0b0000001 => { /* MULHU */
                           return Instr::Mulhu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };

                        },
                        _ => { unreachable!(); }
                    }
                },
                0b100 => {
                    match instr.funct7 {
                        0b0000000 => { /* XOR */
                           return Instr::Xor { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };

                        },
                        0b0000001 => { /* DIV */
                           return Instr::Div { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };

                        },
                        _ => { unreachable!(); }
                    }
                },
                0b101 => {
                    match instr.funct7 {
                        0b0000000 => { /* SRL */
                           return Instr::Srl { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0100000 => { /* SRA */
                           return Instr::Sra { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* DIVU */
                           return Instr::Divu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b110 => {
                    match instr.funct7 {
                        0b0000000 => { /* OR */
                           return Instr::Or { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* REM */
                           return Instr::Rem { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b111 => {
                    match instr.funct7 {
                        0b0000000 => { /* AND */
                           return Instr::And { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* REMU */
                           return Instr::Remu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
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
                return Instr::Ecall;
            } else if instr == 0b00000000000100000000000001110011 { /* EBREAK */
                return Instr::Ebreak;
            } else { unreachable!(); }
        },
        0b0011011 => {
            let instr = IType::new(instr);

            match instr.funct3 {
                0b000 => { /* ADDIW */
                    return Instr::Addiw { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                0b001 => { /* SLLIW */
                    let this_shamt = instr.imm & 0b11111;
                    return Instr::Slliw { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt };
                },
                0b101 => {
                    match (instr.imm >> 5) & 0b1111111 {
                        0b0000000 => { /* SRLIW */
                            let this_shamt = instr.imm & 0b11111;
                            return Instr::Srliw { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt };
                        },
                        0b0100000 => { /* SRAIW */
                            let this_shamt = instr.imm & 0b11111;
                            return Instr::Sraiw { rd: instr.rd, rs1: instr.rs1, shamt: this_shamt };
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
                            return Instr::Addw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0100000 => { /* SUBW */
                            return Instr::Subw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* MULW */
                            return Instr::Mulw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b001 => { /* SLLW */
                    return Instr::Sllw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                0b101 => {
                    match instr.funct7 {
                        0b0000000 => { /* SRLW */
                            return Instr::Srlw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0100000 => { /* SRAW */
                            return Instr::Sraw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        0b0000001 => { /* DIVUW */
                            return Instr::Divuw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                        },
                        _ => { unreachable!(); }
                    }
                },
                0b100 => { /* DIVW */
                    return Instr::Divw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                0b110 => { /* REMW */
                    return Instr::Remw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                0b111 => { /* REMUW */
                    return Instr::Remuw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                _ => { unreachable!(); }
            }
        },
        _ => { panic!("Something went wrong: {:x?}", instr); }
    }
    return Instr::Undefined;
}
