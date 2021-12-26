mod mmu;
mod emulator;

use emulator::{
    Emulator, 
    Register::{Zero, Ra, Sp, Gp, Tp, T0, T1, T2, S0, S1, A0, A1, A2, A3, A4, A5, A6, A7, S2, S3, S4,
                S5, S6, S7, S8, S9, S10, S11, T3, T4, T5, T6, Pc},
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    io,
    thread,
};

const ELFMAGIC:     u32 = 0x464c457f;
const ARCH64:       u8  = 0x2;
const LITTLEENDIAN: u8  = 0x1;
const TYPEEXEC:     u16 = 0x2;
const RISCV:        u16 = 0x3f;
const LOADSEGMENT:  u32 = 0x1;

#[derive(Copy, Clone, Default)]
pub struct Header {
	pub magic:             u32,
	pub bitsize:           u8, // Only supporting 64-bit
	pub endian:            u8, // Only supporting Little Endian
	pub ident_abi_version: u8,
	pub target_platform:   u8,
	pub abi_version:       u8,
	pub padding:           [u8; 7],
	pub o_type:            u16,
	pub machine:           u16, // RISCV-V is 0xf3
	pub version:           u32,
	pub entry_addr:        usize, // Program entry point
	pub phoff:             usize, // Program Header Offset
	pub shoff:             usize, // Section Header Offset
	pub flags:             u32,
	pub ehsize:            u16,
	pub phentsize:         u16,
	pub phnum:             u16, // Number of Program Headers
	pub shentsize:         u16,
	pub shnum:             u16,
	pub shstrndx:          u16,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ProgramHeader {
	pub seg_type: u32,
	pub flags:    u32,
	pub offset:   usize,
	pub vaddr:    usize,
	pub paddr:    usize,
	pub filesz:   usize,
	pub memsz:    usize,
	pub align:    usize,
}

fn parse_header(mut binary: &[u8]) -> Header {
    let mut elf_hdr = Header::default();

    elf_hdr.magic             = binary.read_u32::<LittleEndian>().unwrap();
    elf_hdr.bitsize           = binary.read_u8::<>().unwrap();
    elf_hdr.endian            = binary.read_u8::<>().unwrap();
    elf_hdr.ident_abi_version = binary.read_u8::<>().unwrap();
    elf_hdr.target_platform   = binary.read_u8::<>().unwrap();
    elf_hdr.abi_version       = binary.read_u8::<>().unwrap();
    elf_hdr.padding.map(|_| binary.read_u8::<>().unwrap());
    elf_hdr.o_type            = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.machine           = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.version           = binary.read_u32::<LittleEndian>().unwrap();
    elf_hdr.entry_addr        = binary.read_u64::<LittleEndian>().unwrap() as usize;
    elf_hdr.phoff             = binary.read_u64::<LittleEndian>().unwrap() as usize;
    elf_hdr.shoff             = binary.read_u64::<LittleEndian>().unwrap() as usize;
    elf_hdr.flags             = binary.read_u32::<LittleEndian>().unwrap();
    elf_hdr.ehsize            = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.phentsize         = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.phnum             = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.shentsize         = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.shnum             = binary.read_u16::<LittleEndian>().unwrap();
    elf_hdr.shstrndx          = binary.read_u16::<LittleEndian>().unwrap();

    if elf_hdr.magic   != ELFMAGIC     { panic!("Magic value does not match ELF magic"); }
    if elf_hdr.bitsize != ARCH64       { panic!("Architecture is not 64-bit"); }
    if elf_hdr.endian  != LITTLEENDIAN { panic!("Endian is not Little Endian"); }
    if elf_hdr.o_type  != TYPEEXEC     { panic!("Elf is not an executeable"); }

    elf_hdr
}

fn parse_program_header(mut binary: &[u8]) -> ProgramHeader {
    let mut program_header = ProgramHeader::default();

    program_header.seg_type = binary.read_u32::<LittleEndian>().unwrap();
    program_header.flags    = binary.read_u32::<LittleEndian>().unwrap();
    program_header.offset   = binary.read_u64::<LittleEndian>().unwrap() as usize;
    program_header.vaddr    = binary.read_u64::<LittleEndian>().unwrap() as usize;
    program_header.paddr    = binary.read_u64::<LittleEndian>().unwrap() as usize;
    program_header.filesz   = binary.read_u64::<LittleEndian>().unwrap() as usize;
    program_header.memsz    = binary.read_u64::<LittleEndian>().unwrap() as usize;
    program_header.align    = binary.read_u64::<LittleEndian>().unwrap() as usize;

    program_header
}

fn elf_loader(filename: &str, emu_inst: &mut Emulator) -> Option<()> {
    let target = std::fs::read(filename).expect("Error loading target binary");
    let mut program_hdr;
    let elf_hdr = parse_header(&target);

    let mut offset = elf_hdr.phoff - elf_hdr.phentsize as usize;
    for i in 0..elf_hdr.phnum {
        offset += elf_hdr.phentsize as usize;
        program_hdr = parse_program_header(&target[offset..]);

        if program_hdr.seg_type != LOADSEGMENT { continue; }

        emu_inst.load_section(program_hdr, &target[program_hdr.offset..
                              program_hdr.offset.checked_add(program_hdr.memsz)?]);
        //println!("{:#x?}", program_hdr);
    }

    emu_inst.set_reg(Pc, elf_hdr.entry_addr);

    //println!("magic: 0x{:x?}", elf_hdr.magic);
    //println!("entry_addr: 0x{:x?}", elf_hdr.entry_addr);
    //println!("program header offset: {:?}", elf_hdr.phoff);
    //println!("section header offset: {:?}", elf_hdr.shoff);
    //println!("program header num: {:?}", elf_hdr.phnum);
    //println!("machine: {:x?}", elf_hdr.machine);
    //println!("type: {:x?}", elf_hdr.o_type);

    Some(())
}

fn worker(thr_id: usize, mut emu: Emulator) {
    let original = emu.clone();
    loop {
        emu = original.clone();
        emu.run_emu();
    }
}

fn main() -> io::Result<()> {
    let mut emu_inst= Emulator::new(32 * 1024 * 1024);
    elf_loader("./test_bin", &mut emu_inst);

    // Setup Stack
    let stack = emu_inst.allocate(1024 * 1024).expect("Error allocating stack");
    emu_inst.set_reg(Sp, stack + 1024 * 1024);
    // TODO

    for thr_id in 0..1 {
        let emu = emu_inst.clone();
        thread::spawn(move || worker(thr_id, emu));
    }

    loop {
    }
}
