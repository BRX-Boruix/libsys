//! 系统调用底层入口：`int 0x80` 软中断 + 参数寄存器约定。
//!
//! 与内核 `syscall.rs` 对齐：
//! - `rax = 系统调用号`，`rdi/rsi/rdx/r10/r8/r9 = a1..a6`；
//! - 返回 `rax` 全 64 位结果；错误时 `bit63` 置位（`-errno` 补码）。

use crate::error::Error;

/// 触发一次系统调用，返回原始 `rax`（不解包错误位）。
///
/// `int 0x80` 经 IDT 进入内核，内核 handler 保存/恢复全部通用寄存器，
/// 仅 `rax`（返回值）被改写。声明 `rcx`/`r11` 为 clobber 是保守做法
/// （软中断不会像 `syscall` 指令那样用它们，但保持 ABI 稳健）。
#[inline]
pub fn invoke(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") nr as u64 => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// 触发系统调用并解包为 Rust 风格 `Result<u64, Error>`。
///
/// `bit63` 置位 = 错误，取 `-ret` 得 errno，映射回 [`Error`]。
#[inline]
pub fn call(nr: u32, args: [u64; 6]) -> Result<u64, Error> {
    let ret = invoke(nr, args[0], args[1], args[2], args[3], args[4], args[5]);
    if ret & (1u64 << 63) != 0 {
        Err(Error::from_errno((ret as i64).wrapping_neg() as i32))
    } else {
        Ok(ret)
    }
}
