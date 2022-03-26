use crate::mmu::Perms;
use crate::emulator::{Emulator, Register, STDOUT, STDERR, Fault};

// Helper Structs for syscalls {{{

//#[repr(C)]
//#[derive(Default, Debug)]
//struct Stat {
//    st_dev:     u64, // dev_t
//    st_ino:     u64, // ino_t
//    st_rdev:    u64, // dev_t
//    st_size:    i64, // off_t
//    st_blocks:  u64, // st_blocks
//
//    st_mode:    u32, // mode_t
//    st_uid:     u32, // uid_t
//    st_gid:     u32, // gid_t
//    st_blksize: u32, // blksize_t
//    st_nlink:   u32, // nlink_t
//    __pad0:     u32,
//
//    st_atime_sec: u64,
//    st_atimensec: u64,
//    st_mtime_sec: u64,
//    st_mtimensec: u64,
//    st_ctime_sec: u64,
//    st_ctimensec: u64,
//
//    __glibc_reserved: [i64; 3],
//}

#[repr(C)]
#[derive(Default, Debug)]
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
    let fd = emu.get_reg(Register::A0) as usize;
    let _statbuf = emu.get_reg(Register::A1);

    let file = emu.fd_list.get(fd);

    if file.is_none() {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    let _file = file.unwrap();

    // For now just return -1
    emu.set_reg(Register::A0, 0);

    None
}

//pub fn fstat(emu: &mut Emulator) -> Option<Fault> {
//    let fd      = emu.get_reg(Register::A0) as usize;
//    let statbuf = emu.get_reg(Register::A1);
//
//    // Check if the FD is valid
//    let file = emu.fd_list.get(fd);
//    if file.is_none() {
//        // FD was not valid, return out with an error
//        emu.set_reg(Register::A0, !0);
//        return None;
//    }
//
//    let mut stat = Stat::default();
//    stat.st_dev = 0x803;
//    stat.st_ino = 0x81889;
//    stat.st_mode = 0x81a4;
//    stat.st_nlink = 0x1;
//    stat.st_uid = 0x3e8;
//    stat.st_gid = 0x3e8;
//    stat.st_rdev = 0x0;
//    stat.st_size = 0x4444;
//    stat.st_blksize = 0x1000;
//    stat.st_blocks = 0x5555;
//    stat.st_atime = 0x5f0fe246;
//    stat.st_mtime = 0x5f0fe244;
//    stat.st_ctime = 0x5f0fe244;
//
//    // Cast the stat structure to raw bytes
//    let stat = unsafe {
//        core::slice::from_raw_parts(
//            &stat as *const Stat as *const u8,
//            core::mem::size_of_val(&stat))
//    };
//
//    // Write in the stat data
//    emu.memory.write_mem(statbuf as usize, stat, stat.len()).unwrap();
//    emu.set_reg(Register::A0, 0);
//
//    None
//}

pub fn write(emu: &mut Emulator) -> Option<Fault> {
    let fd    = emu.get_reg(Register::A0) as usize;
    let buf   = emu.get_reg(Register::A1);
    let count = emu.get_reg(Register::A2);

    let file = emu.fd_list.get_mut(fd);

    if file.is_none() {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    if true {
        let file = file.unwrap();
        if *file == STDOUT || *file == STDERR {
            let mut read_data = vec![0u8; count];
            emu.memory.read_into(buf, &mut read_data, count, Perms::READ).unwrap();
            let s = std::str::from_utf8(&read_data);

            print!("{}", s.unwrap());
        } else {
            panic!("Write to unsupported file occured");
        }
    }

    emu.set_reg(Register::A0, count);
    None
}

pub fn brk(emu: &mut Emulator) -> Option<Fault> {
    let base = emu.get_reg(Register::A0);
    if base != 0 {
        //panic!("brk not implemented for non 0 base address");
    }

    emu.set_reg(Register::A0, 0);
    panic!("brk not yet implemented");
}

pub fn close(emu: &mut Emulator) -> Option<Fault> {
    let fd = emu.get_reg(Register::A0) as usize;

    let file = emu.fd_list.get_mut(fd);

    if file.is_none() {
        emu.set_reg(Register::A0, !0);
        return None;
    }

    let file = file.unwrap();

    *file = -1;

    emu.set_reg(Register::A0, 0);
    None
}
