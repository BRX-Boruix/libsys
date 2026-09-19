//! 系统调用底层入口：`syscall`/`sysret` 快速路径 + `int 0x80` 兼容兜底。
//!
//! 与内核 `syscall.rs` 对齐：
//! - `rax = 系统调用号`，`rdi/rsi/rdx/r10/r8/r9 = a1..a6`；
//! - 返回 `rax` 全 64 位结果；错误时 `bit63` 置位（`-errno` 补码）。
//!
//! # 两条入口路径（SYSCALL-FAST-4）
//!
//! | 路径 | 指令 | 进入方式 |
//! |---|---|---|
//! | 快速 | `syscall` | 走 `IA32_LSTAR`，内核自建帧 + `sysretq` 返回 |
//! | 兜底 | `int 0x80` | 走 IDT 陷阱门，硬件压帧 + `iretq` 返回 |
//!
//! 两条路径的**参数约定与返回值语义完全相同**，故上层无需感知差异。
//! 默认走快速路径；`int 0x80` 保留为兼容兜底（老硬件、调试、快速路径异常时回退）。
//!
//! # `syscall` 指令的 ABI 差异（必须显式处理）
//!
//! `syscall` 硬件会用 `rcx` 存返回 RIP、`r11` 存 RFLAGS，故这两个寄存器**必然**
//! 被破坏——不是可选项。同时 `r10` 是 a4，与 `sysretq` 的返回约定无关（r10 不被
//! 硬件消耗），故 a4 传递方式不变。
//!
//! 本模块统一把 `rcx`/`r11` 声明为 clobber：
//!   - `syscall` 路径下它们确实被硬件改写；
//!   - `int 0x80` 路径下软中断不碰它们，但内核 handler 会保存/恢复全部寄存器，
//!     所以声明 clobber 是**保守且正确**的（原实现即如此）。
//! 两条路径共用同一份 clobber 列表，避免因路径不同而产生 ABI 分歧。

use crate::error::Error;

/// 系统调用入口实现的选择。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryKind {
    /// `syscall`/`sysret` 快速路径。
    Syscall,
    /// `int 0x80` 软中断兜底路径。
    Int80,
}

/// 当前生效的入口实现。
///
/// 初始为 [`EntryKind::Syscall`]；若快速路径不可用（例如内核未配置 MSR，或运行在
/// 无 `syscall` 指令的 CPU 上），由 [`set_entry`] 切到 [`EntryKind::Int80`]。
/// 当前入口实现存储（原子；`0` = Syscall，`1` = Int80）。
///
/// 用 `AtomicU8` 而非 `static mut`：用户态虽然单线程，但入口选择是**进程级全局
/// 状态**，且 `static mut` 的读写在 Rust 2024 中已被明确限制（`static_mut_refs`）；
/// 用原子可让读路径无 unsafe、无数据竞争嫌疑。
static ENTRY: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// 设置入口实现。
///
/// 不标 `unsafe`：切换本身不涉及内存安全——`EntryKind` 只有两个合法值，
/// 两种指令都在同一 ABI 下。**但调用方仍须保证语义前提**：切到
/// [`EntryKind::Syscall`] 前，内核必须已配置 `IA32_LSTAR`/`STAR`/`FMASK`/`EFER.SCE`，
/// 否则 `syscall` 会跳到未定义地址。该前提由内核启动期保证（见 `kmain`）。
pub fn set_entry(kind: EntryKind) {
    let v = match kind {
        EntryKind::Syscall => 0u8,
        EntryKind::Int80 => 1u8,
    };
    ENTRY.store(v, core::sync::atomic::Ordering::Release);
}

/// 读取当前入口实现。
#[inline]
pub fn entry() -> EntryKind {
    if ENTRY.load(core::sync::atomic::Ordering::Acquire) == 0 {
        EntryKind::Syscall
    } else {
        EntryKind::Int80
    }
}

/// 触发一次系统调用，返回原始 `rax`（不解包错误位）。
#[inline]
pub fn invoke(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
    match entry() {
        EntryKind::Syscall => invoke_syscall(nr, a1, a2, a3, a4, a5, a6),
        EntryKind::Int80 => invoke_int80(nr, a1, a2, a3, a4, a5, a6),
    }
}

/// `syscall` 快速路径。
#[inline]
fn invoke_syscall(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") nr as u64 => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            // syscall 硬件用 rcx 存返回 RIP、r11 存 RFLAGS——必然被破坏。
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// `int 0x80` 兜底路径。
#[inline]
fn invoke_int80(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
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

/// 触发一次系统调用并额外捕获返回帧 `r10`（被收尸子进程 pid 的交付通道）。
///
/// 仅 waitpid 使用：内核把退出码写 `rax`、被收尸子进程 pid 写 `r10`（同步路径
/// 经 `SyscallFrame.aux_pid` 由架构层写回，阻塞路径经 `saved.r10` 交付）。返回
/// `(ret, r10)` 原始值，错误位由调用方解包。
///
/// # 两条路径下 `r10` 的语义（SYSCALL-FAST-3 已实核）
///
/// `r10` 在 `syscall` ABI 下兼作 a4。但 `waitpid` 内核侧**不读取 a4**，且
/// `sysretq` 不消耗 r10，故两条路径都能把 pid 带回。该约定由内核侧
/// `test_syscall_fast3_r10_capture_contract` 锁定。
#[inline]
pub fn invoke_capture_r10(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> (u64, u64) {
    match entry() {
        EntryKind::Syscall => capture_syscall(nr, a1, a2, a3, a4, a5, a6),
        EntryKind::Int80 => capture_int80(nr, a1, a2, a3, a4, a5, a6),
    }
}

#[inline]
fn capture_syscall(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> (u64, u64) {
    let ret: u64;
    let r10_out: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") nr as u64 => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            lateout("r10") r10_out,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    (ret, r10_out)
}

#[inline]
fn capture_int80(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> (u64, u64) {
    let ret: u64;
    let r10_out: u64;
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
            lateout("r10") r10_out,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    (ret, r10_out)
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