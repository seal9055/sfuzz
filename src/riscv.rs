use crate::emulator::Register;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Instr {
    Undefined,
    Ecall,
    Ebreak,
    Lui    { rd: Register, imm: i32 },
    Auipc  { rd: Register, imm: i32 },
    Jal    { rd: Register, imm: i32 },
    Jalr   { rd: Register, rs1: Register, imm: i32 },
    Beq    { rs1: Register, rs2: Register, imm: i32, mode: u8},
    Bne    { rs1: Register, rs2: Register, imm: i32,  mode: u8},
    Blt    { rs1: Register, rs2: Register, imm: i32,  mode: u8},
    Bge    { rs1: Register, rs2: Register, imm: i32,  mode: u8},
    Bltu   { rs1: Register, rs2: Register, imm: i32,  mode: u8},
    Bgeu   { rs1: Register, rs2: Register, imm: i32,  mode: u8},
    Lb     { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Lh     { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Lw     { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Lbu    { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Lhu    { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Sb     { rs1: Register, rs2: Register, imm: i32, mode: u8 },
    Sh     { rs1: Register, rs2: Register, imm: i32, mode: u8 },
    Sw     { rs1: Register, rs2: Register, imm: i32, mode: u8 },
    Addi   { rd: Register, rs1: Register, imm: i32 },
    Slti   { rd: Register, rs1: Register, imm: i32 },
    Sltiu  { rd: Register, rs1: Register, imm: i32 },
    Xori   { rd: Register, rs1: Register, imm: i32 },
    Ori    { rd: Register, rs1: Register, imm: i32 },
    Andi   { rd: Register, rs1: Register, imm: i32 },
    Add    { rd: Register, rs1: Register, rs2: Register },
    Sub    { rd: Register, rs1: Register, rs2: Register },
    Sll    { rd: Register, rs1: Register, rs2: Register },
    Slt    { rd: Register, rs1: Register, rs2: Register },
    Sltu   { rd: Register, rs1: Register, rs2: Register },
    Xor    { rd: Register, rs1: Register, rs2: Register },
    Srl    { rd: Register, rs1: Register, rs2: Register },
    Sra    { rd: Register, rs1: Register, rs2: Register },
    Or     { rd: Register, rs1: Register, rs2: Register },
    And    { rd: Register, rs1: Register, rs2: Register },
    Lwu    { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Ld     { rd: Register, rs1: Register, imm: i32, mode: u8 },
    Sd     { rs1: Register, rs2: Register, imm: i32, mode: u8 },
    Slli   { rd: Register, rs1: Register, shamt: i32 },
    Srli   { rd: Register, rs1: Register, shamt: i32 },
    Srai   { rd: Register, rs1: Register, shamt: i32 },
    Addiw  { rd: Register, rs1: Register, imm: i32 },
    Slliw  { rd: Register, rs1: Register, shamt: i32 },
    Srliw  { rd: Register, rs1: Register, shamt: i32 },
    Sraiw  { rd: Register, rs1: Register, shamt: i32 },
    Addw   { rd: Register, rs1: Register, rs2: Register },
    Subw   { rd: Register, rs1: Register, rs2: Register },
    Sllw   { rd: Register, rs1: Register, rs2: Register },
    Srlw   { rd: Register, rs1: Register, rs2: Register },
    Sraw   { rd: Register, rs1: Register, rs2: Register },
    Mul    { rd: Register, rs1: Register, rs2: Register },
    Mulh   { rd: Register, rs1: Register, rs2: Register },
    Mulhsu { rd: Register, rs1: Register, rs2: Register },
    Mulhu  { rd: Register, rs1: Register, rs2: Register },
    Div    { rd: Register, rs1: Register, rs2: Register },
    Divu   { rd: Register, rs1: Register, rs2: Register },
    Rem    { rd: Register, rs1: Register, rs2: Register },
    Remu   { rd: Register, rs1: Register, rs2: Register },
    Mulw   { rd: Register, rs1: Register, rs2: Register },
    Divw   { rd: Register, rs1: Register, rs2: Register },
    Divuw  { rd: Register, rs1: Register, rs2: Register },
    Remw   { rd: Register, rs1: Register, rs2: Register },
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
        UType {
            //imm: ((val << 12) >> 12) as i32,
            imm: (instr & !0xfff) as i32,
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
            imm: ((val as i32)<< 11) >> 11,
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
                    return
                        Instr::Beq { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b000 };
                },
                0b001 => { /* BNE */
                    return
                        Instr::Bne { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b001 };
                },
                0b100 => { /* BLT */
                    return
                        Instr::Blt { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b100 };
                },
                0b101 => { /* BGE */
                    return
                        Instr::Bge { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b101 };
                },
                0b110 => { /* BLTU */
                    return
                        Instr::Bltu { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b110 };
                },
                0b111 => { /* BGEU */
                    return
                        Instr::Bgeu { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b111 };
                },
                _ => { unreachable!(); }
            }

        },
        0b0000011 => {
            let instr = IType::new(instr);
            match instr.funct3 {
                0b000 => { /* LB */
                    return Instr::Lb { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b000 };
                },
                0b001 => { /* LH */
                    return Instr::Lh { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b001};
                },
                0b010 => { /* LW */
                    return Instr::Lw { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b010};
                },
                0b100 => { /* LBU */
                    return Instr::Lbu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b100 };
                },
                0b101 => { /* LHU */
                    return Instr::Lhu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b101 };
                },
                0b110 => { /* LWU */
                    return Instr::Lwu { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b110 };
                },
                0b011 => { /* LD */
                    return Instr::Ld { rd: instr.rd, rs1: instr.rs1, imm: instr.imm, mode: 0b011};
                },
                _ => { unreachable!(); }
            }
        },
        0b0100011 => {
            let instr = SType::new(instr);
            match instr.funct3 {
                0b000 => { /* SB */
                    return
                        Instr::Sb { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b000 };
                },
                0b001 => { /* SH */
                    return
                        Instr::Sh { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b001 };
                },
                0b010 => { /* SW */
                    return
                        Instr::Sw { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b010 };
                },
                0b011 => { /* SD */
                    return
                        Instr::Sd { rs1: instr.rs1, rs2: instr.rs2, imm: instr.imm, mode: 0b011 };
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
                    let shamt = instr.imm & 0b111111;
                    return Instr::Slli { rd: instr.rd, rs1: instr.rs1, shamt};
                },
                0b101 => {
                    match (instr.imm >> 6) & 0b111111 {
                        0b000000 => { /* SRLI */
                            let shamt = instr.imm & 0b111111;
                            return Instr::Srli { rd: instr.rd, rs1: instr.rs1, shamt };
                        },
                        0b010000 => { /* SRAI */
                            let shamt = instr.imm & 0b111111;
                            return Instr::Srai { rd: instr.rd, rs1: instr.rs1, shamt };
                        },
                        _ => { unreachable!(); }
                    }
                },
                _ => { unreachable!(); }
            }
        },
        0b0110011 => {
            let instr = RType::new(instr);
            match (instr.funct3, instr.funct7) {
                (0b000, 0b0000000) => { /* ADD */
                    return Instr::Add { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b000, 0b0100000) => { /* SUB */
                    return Instr::Sub { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b000, 0b0000001) => { /* MUL */
                    return Instr::Mul { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b001, 0b0000000) => { /* SLL */
                    return Instr::Sll { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b001, 0b0000001) => { /* MULH */
                    return Instr::Mulh { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b010, 0b0000000) => { /* SLT */
                    return Instr::Slt { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b010, 0b0000001) => { /* MULHSU */
                    return Instr::Mulhsu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b011, 0b0000000) => { /* SLTU */
                    return Instr::Sltu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b011, 0b0000001) => { /* MULHU */
                    return Instr::Mulhu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b100, 0b0000000) => { /* XOR */
                    return Instr::Xor { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b100, 0b0000001) => { /* DIV */
                    return Instr::Div { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101, 0b0000000) => { /* SRL */
                    return Instr::Srl { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101, 0b0100000) => { /* SRA */
                    return Instr::Sra { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101, 0b0000001) => { /* DIVU */
                    return Instr::Divu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b110, 0b0000000) => { /* OR */
                    return Instr::Or { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b110, 0b0000001) => { /* REM */
                    return Instr::Rem { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b111, 0b0000000) => { /* AND */
                    return Instr::And { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b111, 0b0000001) => { /* REMU */
                    return Instr::Remu { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
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
            let mode = (instr.imm >> 5) & 0b1111111;

            match (instr.funct3, mode) {
                (0b000, _) => { /* ADDIW */
                    return Instr::Addiw { rd: instr.rd, rs1: instr.rs1, imm: instr.imm };
                },
                (0b001, _) => { /* SLLIW */
                    let shamt = instr.imm & 0b11111;
                    return Instr::Slliw { rd: instr.rd, rs1: instr.rs1, shamt};
                },
                (0b101, 0b0000000 ) => { /* SRLIW */
                    let shamt = instr.imm & 0b11111;
                    return Instr::Srliw { rd: instr.rd, rs1: instr.rs1, shamt};
                },
                (0b101, 0b0100000 ) => { /* SRAIW */
                    let shamt = instr.imm & 0b11111;
                    return Instr::Sraiw { rd: instr.rd, rs1: instr.rs1, shamt};
                },
                _ => { unreachable!(); },
            }
        }
        0b0111011 => {
            let instr = RType::new(instr);
            match (instr.funct3, instr.funct7) {
                (0b000,  0b0000000) => { /* ADDW */
                    return Instr::Addw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b000,  0b0100000) => { /* SUBW */
                    return Instr::Subw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b000,  0b0000001) => { /* MULW */
                    return Instr::Mulw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b001, 0b00000000) => { /* SLLW */
                    return Instr::Sllw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101,  0b0000000) => { /* SRLW */
                    return Instr::Srlw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101,  0b0100000) => { /* SRAW */
                    return Instr::Sraw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b101,  0b0000001) => { /* DIVUW */
                    return Instr::Divuw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b100,  0b0000001) => { /* DIVW */
                    return Instr::Divw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b110,  0b0000001) => { /* REMW */
                    return Instr::Remw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                (0b111,  0b0000001) => { /* REMUW */
                    return Instr::Remuw { rd: instr.rd, rs1: instr.rs1, rs2: instr.rs2 };
                },
                _ => { panic!("Instr: {:#?}", instr); }//unreachable!(); }
            }
        },
        _ => { return Instr::Undefined; }
    }
    return Instr::Undefined;
}

/// Unit tests for each Instruction encoding Riscv uses
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined() {
        assert_eq!(decode_instr(0x0), Instr::Undefined)
    }

    #[test]
    fn ecall() {
        assert_eq!(decode_instr(0x73), Instr::Ecall)
    }

    #[test]
    fn ebreak() {
        assert_eq!(decode_instr(0x100073), Instr::Ebreak)
    }

    #[test]
    fn rtype() {
        match decode_instr(0xf70733) { Instr::Add{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A4); assert_eq!(rs2, Register::A5);
            assert_eq!(rs1, Register::A4); }, _ => { panic!(""); } };
        match decode_instr(0x40c685b3) { Instr::Sub{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A1); assert_eq!(rs2, Register::A2);
            assert_eq!(rs1, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0x2f48a33) { Instr::Mul{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::S4); assert_eq!(rs2, Register::A5);
            assert_eq!(rs1, Register::S1); }, _ => { panic!(""); } };
        match decode_instr(0x2f48a33) { Instr::Mul{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::S4); assert_eq!(rs2, Register::A5);
            assert_eq!(rs1, Register::S1); }, _ => { panic!(""); } };
        match decode_instr(0x299f7b3) { Instr::Remu{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A5); assert_eq!(rs2, Register::S1);
            assert_eq!(rs1, Register::S3); }, _ => { panic!(""); } };
        match decode_instr(0x299f7b3) { Instr::Remu{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A5); assert_eq!(rs2, Register::S1);
            assert_eq!(rs1, Register::S3); }, _ => { panic!(""); } };
        match decode_instr(0x2b6c6bb) { Instr::Divw{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A3); assert_eq!(rs2, Register::A1);
            assert_eq!(rs1, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0x2c5b7b3) { Instr::Mulhu{ rd, rs1 , rs2} => {
            assert_eq!(rd, Register::A5); assert_eq!(rs2, Register::A2);
            assert_eq!(rs1, Register::A1); }, _ => { panic!(""); } };
    }

    #[test]
    fn itype() {
        match decode_instr(0x1259583) { Instr::Lh{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A1); assert_eq!(imm, 18);
            assert_eq!(rs1, Register::A1); }, _ => { panic!(""); } };
        match decode_instr(0x1099703) { Instr::Lh{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A4); assert_eq!(imm, 16);
            assert_eq!(rs1, Register::S3); }, _ => { panic!(""); } };
        match decode_instr(0x0ac42683) { Instr::Lw{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A3); assert_eq!(imm, 172);
            assert_eq!(rs1, Register::S0); }, _ => { panic!(""); } };
        match decode_instr(0x3107a883) { Instr::Lw{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A7); assert_eq!(imm, 784);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x01813083) { Instr::Ld{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::Ra); assert_eq!(imm, 24);
            assert_eq!(rs1, Register::Sp); }, _ => { panic!(""); } };
        match decode_instr(0x6714603) { Instr::Lbu{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A2); assert_eq!(imm, 103);
            assert_eq!(rs1, Register::Sp); }, _ => { panic!(""); } };
        match decode_instr(0xd4583) { Instr::Lbu{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A1); assert_eq!(imm, 0);
            assert_eq!(rs1, Register::S10); }, _ => { panic!(""); } };
        match decode_instr(0x1015783) { Instr::Lhu{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A5); assert_eq!(imm, 16);
            assert_eq!(rs1, Register::Sp); }, _ => { panic!(""); } };
        match decode_instr(0x15015783) { Instr::Lhu{ rd, rs1 , imm, mode: _} => {
            assert_eq!(rd, Register::A5); assert_eq!(imm, 336);
            assert_eq!(rs1, Register::Sp); }, _ => { panic!(""); } };
        match decode_instr(0xf98680e7) { Instr::Jalr{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::Ra); assert_eq!(imm, -104);
            assert_eq!(rs1, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0x700e7) { Instr::Jalr{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::Ra); assert_eq!(imm, 0);
            assert_eq!(rs1, Register::A4); }, _ => { panic!(""); } };
        match decode_instr(0xe7) { Instr::Jalr{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::Ra); assert_eq!(imm, 0);
            assert_eq!(rs1, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0xc0070713) { Instr::Addi{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A4); assert_eq!(imm, -1024);
            assert_eq!(rs1, Register::A4); }, _ => { panic!(""); } };
        match decode_instr(0xfff78693) { Instr::Addi{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A3); assert_eq!(imm, -1);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x8307c793) { Instr::Xori{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A5); assert_eq!(imm, -2000);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x807e793) { Instr::Ori{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A5); assert_eq!(imm, 128);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x7ff7f793) { Instr::Andi{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A5); assert_eq!(imm, 2047);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0xfc37071b) { Instr::Addiw{ rd, rs1 , imm} => {
            assert_eq!(rd, Register::A4); assert_eq!(imm, -61);
            assert_eq!(rs1, Register::A4); }, _ => { panic!(""); } };
        match decode_instr(0x4147d69b) { Instr::Sraiw{ rd, rs1 , shamt} => {
            assert_eq!(rd, Register::A3); assert_eq!(shamt, 0x14);
            assert_eq!(rs1, Register::A5); }, _ => { panic!(""); } };
    }


    #[test]
    fn stype() {
        match decode_instr(0xfedd8fa3) { Instr::Sb{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::S11); assert_eq!(imm, -1);
            assert_eq!(rs2, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0x60103a3) { Instr::Sb{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::Sp); assert_eq!(imm, 103);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0xef11023) { Instr::Sh{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::Sp); assert_eq!(imm, 224);
            assert_eq!(rs2, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0xf69023) { Instr::Sh{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A3); assert_eq!(imm, 0);
            assert_eq!(rs2, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x7801a823) { Instr::Sw{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::Gp); assert_eq!(imm, 1936);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0x852023) { Instr::Sw{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A0); assert_eq!(imm, 0);
            assert_eq!(rs2, Register::S0); }, _ => { panic!(""); } };
    }

    #[test]
    fn btype() {
        match decode_instr(0x78c63) { Instr::Beq{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A5); assert_eq!(imm, 0x18);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0xf70c63) { Instr::Beq{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A4); assert_eq!(imm, 0x18);
            assert_eq!(rs2, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x1d041463) { Instr::Bne{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::S0); assert_eq!(imm, 0x1c8);
            assert_eq!(rs2, Register::A6); }, _ => { panic!(""); } };
        match decode_instr(0x2071463) { Instr::Bne{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A4); assert_eq!(imm, 0x28);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0x12d8ce63) { Instr::Blt{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A7); assert_eq!(imm, 0x13c);
            assert_eq!(rs2, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0xfe06c2e3) { Instr::Blt{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A3); assert_eq!(imm, -0x1c);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0x36f6dee3) { Instr::Bge{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A3); assert_eq!(imm, 0xb7c);
            assert_eq!(rs2, Register::A5); }, _ => { panic!(""); } };
        match decode_instr(0x9d463) { Instr::Bge{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::S3); assert_eq!(imm, 0x8);
            assert_eq!(rs2, Register::Zero); }, _ => { panic!(""); } };
        match decode_instr(0xa6eb60e3) { Instr::Bltu{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::S6); assert_eq!(imm, -0x5a0);
            assert_eq!(rs2, Register::A4); }, _ => { panic!(""); } };
        match decode_instr(0x2d76063) { Instr::Bltu{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A4); assert_eq!(imm, 0x20);
            assert_eq!(rs2, Register::A3); }, _ => { panic!(""); } };
        match decode_instr(0xf966fae3) { Instr::Bgeu{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A3); assert_eq!(imm, -0x6c);
            assert_eq!(rs2, Register::S6); }, _ => { panic!(""); } };
        match decode_instr(0x1d7f6e3) { Instr::Bgeu{ rs1, rs2, imm, mode: _} => {
            assert_eq!(rs1, Register::A5); assert_eq!(imm, 0x80c);
            assert_eq!(rs2, Register::T4); }, _ => { panic!(""); } };
    }

    #[test]
    fn utype() {
        match decode_instr(0x22637) { Instr::Lui{ rd, imm } => {
                assert_eq!(rd, Register::A2); assert_eq!(imm, 0x22); }, _ => { panic!(""); } };
        match decode_instr(0x8837) { Instr::Lui{ rd, imm } => {
                assert_eq!(rd, Register::A6); assert_eq!(imm, 0x8); }, _ => { panic!(""); } };
        match decode_instr(0xffffc9b7) { Instr::Lui{ rd, imm } => {
                assert_eq!(rd, Register::S3); assert_eq!(imm, 0xffffc); }, _ => { panic!(""); } };
        match decode_instr(0x14197) { Instr::Auipc{ rd, imm } => {
                assert_eq!(rd, Register::Gp); assert_eq!(imm, 0x14); }, _ => { panic!(""); } };
        match decode_instr(0x97) { Instr::Auipc{ rd, imm } => {
                assert_eq!(rd, Register::Ra); assert_eq!(imm, 0x0); }, _ => { panic!(""); } };
        match decode_instr(0xe517) { Instr::Auipc{ rd, imm } => {
                assert_eq!(rd, Register::A0); assert_eq!(imm, 0xe); }, _ => { panic!(""); } };
    }

    #[test]
    fn jtype() {
        match decode_instr(0x7a0000ef) { Instr::Jal{ rd, imm } => {
                assert_eq!(rd, Register::Ra); assert_eq!(imm, 0x7a0); }, _ => { panic!(""); } };
        match decode_instr(0x428010ef) { Instr::Jal{ rd, imm } => {
                assert_eq!(rd, Register::Ra); assert_eq!(imm, 0x1428); }, _ => { panic!(""); } };
        match decode_instr(0x358010ef) { Instr::Jal{ rd, imm } => {
                assert_eq!(rd, Register::Ra); assert_eq!(imm, 0x1358); }, _ => { panic!(""); } };
        match decode_instr(0xf6dff06f) { Instr::Jal{ rd, imm } => {
                assert_eq!(rd, Register::Zero); assert_eq!(imm, -0x94); }, _ => { panic!(""); } };
    }
}
