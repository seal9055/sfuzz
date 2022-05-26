use crate::mmu::Perms;
use crate::emulator::{Emulator, Register, FileType::{self, STDOUT, STDERR, INVALID}, Fault};

// Helper Structs for syscalls {{{

#[repr(C)]
#[derive(Debug)]
struct Stat {
    st_dev:     u64,
    st_ino:     u64,
    st_mode:    u32,
    st_nlink:   u32,
    st_uid:     u32,
    st_gid:     u32,
    st_rdev:    u64,
    __pad1:     u64,

    st_size:    i64,
    st_blksize: i32,
    __pad2:     i32,

    st_blocks: i64,

    st_atime:     u64,
    st_atimensec: u64,
    st_mtime:     u64,
    st_mtimensec: u64,
    st_ctime:     u64,
    st_ctimensec: u64,

    __glibc_reserved: [i32; 2],
}

// }}}


pub fn exit() -> Option<Fault> {
    Some(Fault::Exit)
}

pub fn fstat(emu: &mut Emulator) -> Option<Fault> {
    let fd      = emu.get_reg(Register::A0) as usize;
    let statbuf = emu.get_reg(Register::A1);

    // Check if the FD is valid
    let file = emu.fd_list.get(fd);
    if file.is_none() {
        // FD was not valid, return out with an error
        emu.set_reg(Register::A0, !0);
        return None;
    }

    // qemu output for the syscall + correct input lengths
    if file.unwrap().ftype == FileType::FUZZINPUT {
        let stat: Stat = Stat {
            st_dev:           0x803,
            st_ino:           0x81889,
            st_mode:          0x81a4,
            st_nlink:         0x1,
            st_uid:           0x3e8,
            st_gid:           0x3e8,
            st_rdev:          0x0,
            __pad1:           0,
            st_size:          emu.fuzz_input.len() as i64,
            st_blksize:       0x1000,
            __pad2:           0,
            st_blocks:        (emu.fuzz_input.len() as i64 + 511) / 512,
            st_atime:         0x5f0fe246,
            st_atimensec:     0,
            st_mtime:         0x5f0fe244,
            st_mtimensec:     0,
            st_ctime:         0x5f0fe244,
            st_ctimensec:     0,
            __glibc_reserved: [0, 0],
        };

        // Cast the stat structure to raw bytes
        let stat = unsafe {
            core::slice::from_raw_parts(
                &stat as *const Stat as *const u8,
                core::mem::size_of_val(&stat))
        };

        // Write in the stat data
        emu.memory.write_mem(statbuf as usize, stat, stat.len()).unwrap();
        emu.set_reg(Register::A0, 0);
    } else if file.unwrap().ftype != FileType::OTHER {
        emu.set_reg(Register::A0, !0);
    } else {
        unreachable!();
    }

    None
}

pub fn lseek(emu: &mut Emulator) -> Option<Fault> {
    let fd     = emu.get_reg(Register::A0) as usize;
    let offset = emu.get_reg(Register::A1) as i64;
    let whence = emu.get_reg(Register::A2) as i32;

    if emu.fd_list.len() < fd || emu.fd_list[fd].ftype == FileType::INVALID {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    if emu.fd_list[fd].ftype == FileType::FUZZINPUT {
        let cur = emu.fd_list[fd].cursor.unwrap();

        let new_pos: i64 = match whence {
            0 => offset,                                // SEEK_SET
            1 => cur as i64 + offset,                   // SEEK_CUR
            2 => (emu.fuzz_input.len() as i64) + offset,         // SEEK_END
            _ => {
                emu.set_reg(Register::A0, !0);
                return None;
            }
        };

        let new_pos = core::cmp::max(0i64, new_pos);
        let new_pos = core::cmp::min(new_pos, emu.fuzz_input.len() as i64) as usize;

        emu.fd_list[fd].cursor = Some(new_pos);
        emu.set_reg(Register::A0, new_pos);
    } else {
        unreachable!();
    }
    None
}

pub fn open(emu: &mut Emulator) -> Option<Fault> {
    let filename = emu.get_reg(Register::A0) as usize;
    let _flags    = emu.get_reg(Register::A1);
    let _mode    = emu.get_reg(Register::A2);

    let mut buf: Vec<u8> = Vec::new();
    let mut cur = 0;
    // get filename length
    loop {
        let c: u8 = emu.memory.read_at(filename + cur, Perms::READ).unwrap();
        buf.push(c);
        if c == 0 {
            break;
        }
        cur += 1;
    }

    if buf == b"fuzz_input\0" {
        emu.alloc_file(FileType::FUZZINPUT);
    } else {
        emu.alloc_file(FileType::OTHER);
    }

    emu.set_reg(Register::A0, emu.fd_list.len()-1);
    None
}

pub fn read(emu: &mut Emulator) -> Option<Fault> {
    let fd    = emu.get_reg(Register::A0) as usize;
    let buf   = emu.get_reg(Register::A1);
    let count = emu.get_reg(Register::A2);

    // If the fd does not exist, return an error
    if emu.fd_list.len() < fd || emu.fd_list[fd].ftype == FileType::INVALID {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    // Special case, reading in the fuzzinput
    if emu.fd_list[fd].ftype == FileType::FUZZINPUT {

        let offset = emu.fd_list[fd].cursor.unwrap();
        let len = core::cmp::min(count, emu.fuzz_input.len()-offset);

        emu.memory.write_mem(buf, &emu.fuzz_input[offset..offset+len], len)
            .expect("Error occured while trying to read in fuzz-input");

        emu.set_reg(Register::A0, len);
        emu.fd_list[fd].cursor = Some(offset + len);
    } else {
        // Read in a different file
        //unreachable!();
        emu.set_reg(Register::A0, count);
    }

    None
}

pub fn write(emu: &mut Emulator) -> Option<Fault> {
    let fd    = emu.get_reg(Register::A0) as usize;
    let buf   = emu.get_reg(Register::A1);
    let count = emu.get_reg(Register::A2);

    let file = emu.fd_list.get_mut(fd);

    if file.is_none() {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    if false {
        let file = file.unwrap();
        if file.ftype == STDOUT || file.ftype == STDERR {
            let mut read_data = vec![0u8; count];
            emu.memory.read_into(buf, &mut read_data, count, Perms::READ).unwrap();

            match std::str::from_utf8(&read_data) {
                Ok(v) => print!("{}", v),
                Err(_) => print!("{:?}", read_data),
            }
        } else {
            panic!("Write to unsupported file occured");
        }
    }

    emu.set_reg(Register::A0, count);
    None
}

pub fn brk(emu: &mut Emulator) -> Option<Fault> {
    let base = emu.get_reg(Register::A0);
    if base == 0 {
        emu.set_reg(Register::A0, 0);
        return None;
    }

    panic!("Not supporting brk");
}

pub fn gettimeofday(emu: &mut Emulator) -> Option<Fault> {
    emu.set_reg(Register::A0, 20);
    None
}

pub fn close(emu: &mut Emulator) -> Option<Fault> {
    let fd = emu.get_reg(Register::A0) as usize;

    let file = emu.fd_list.get_mut(fd);

    if file.is_none() {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    let file = file.unwrap();

    file.ftype = INVALID;

    emu.set_reg(Register::A0, 0);
    None
}
