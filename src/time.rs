//! 时间（TIME 域）封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::SYS_TASK_WAIT;

/// `now()`：单调时钟，纳秒（现代 x86_64 直接走 RDTSC / 0 系统调用开销）。
pub fn now() -> u64 {
    let tsc: u64;
    unsafe {
        core::arch::asm!(
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") tsc,
            out("rdx") _,
            options(nomem, nostack)
        );
    }
    // 假设 1GHz (1 tick ≈ 1 ns)，提供纳秒单调时钟
    tsc
}

/// `sleep(ns)`：睡眠指定纳秒（走 SYS_TASK_WAIT(0, ns)）。
pub fn sleep(ns: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_TASK_WAIT, [0, ns, 0, 0, 0, 0]).map(|_| ())
}
