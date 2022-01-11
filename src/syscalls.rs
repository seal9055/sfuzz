use crate::emulator::Fault;
use crate::emulator::{Emulator, Register};

// Helper Structs for syscalls {{{

#[repr(C)]
#[derive(Default, Debug)]
struct Stat {
    st_dev:     u64, // dev_t
    st_ino:     u64, // ino_t
    st_rdev:    u64, // dev_t
    st_size:    i64, // off_t
    st_blocks:  u64, // st_blocks

    st_mode:    u32, // mode_t
    st_uid:     u32, // uid_t
    st_gid:     u32, // gid_t
    st_blksize: u32, // blksize_t
    st_nlink:   u32, // nlink_t
    __pad0:     u32,

    st_atime_sec: u64,
    st_atimensec: u64,
    st_mtime_sec: u64,
    st_mtimensec: u64,
    st_ctime_sec: u64,
    st_ctimensec: u64,

    __glibc_reserved: [i64; 3],
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

pub fn brk(emu: &mut Emulator) -> Option<Fault> {
    let base = emu.get_reg(Register::A0);
    if base != 0 {
        //panic!("brk not implemented for non 0 base address");
    }

    emu.set_reg(Register::A0, 0);
    return None;
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
