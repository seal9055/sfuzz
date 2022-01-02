arr = ["Undefined", "Ecall", "Ebreak", "Lui", "Auipc", "Jal", "Jalr", "Beq", "Bne", "Blt", "Bge",
        "Bltu", "Bgeu", "Lb", "Lh", "Lw", "Lbu", "Lhu", "Sb", "Sh", "Sw", "Addi", "Slti", "Sltiu",
        "Xori", "Ori", "Andi", "Add", "Sub", "Sll", "Slt", "Sltu", "Xor", "Srl", "Sra", "Or", "And",
        "Lwu", "Ld", "Sd", "Slli", "Srli", "Srai", "Addiw", "Slliw", "Srliw", "Sraiw", "Addw",
        "Subw", "Sllw", "Srlw", "Sraw", "Mul", "Mulh", "Mulhsu", "Mulhu", "Div", "Divu", "Rem",
        "Remu", "Mulw", "Divw", "Divuw", "Remw", "Remuw"]

for c in arr:
    print("#[test]")
    print(f"fn {c}()", end="")
    print(" {")
    print("    assert_eq!(decode_instr(0x), )")
    print("}")
    print("")
