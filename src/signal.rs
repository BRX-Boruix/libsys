//! 信号机制用户态封装（ADR-034 PROPOSED / SIGNAL 0x80 域）。
//!
//! - 信号号常量与内核 `task::signals` 对齐（单点定义在各自 crate，双侧数值一致，
//!   S13）。
//! - `action`（sigaction）/`mask`（sigprocmask）经 SIGNAL 域 syscall 封装；
//!   `raise` 复用 `kill(getpid(), sig)`（ADR-034 §2.3，不新增 SIGNAL_SEND）。
//!
//! 值域约束：`mask` 的 `set`（屏蔽位图）与返回的旧屏蔽集均为 bit63 清零的
//! 非负 u64（syscall 错误编码用 bit63，见 `syscall::call`）。信号号 < 64，
//! 恒满足。

use crate::error::Error;
use crate::nr::{
    SYS_SIGNAL_ACTION, SYS_SIGNAL_ALARM, SYS_SIGNAL_ALTSTACK, SYS_SIGNAL_MASK, SYS_SIGNAL_PENDING,
    SYS_SIGNAL_SUSPEND,
};

// ---------- 信号号常量（与内核 task::signals 对齐，S13） ----------

/// SIGINT：终端中断（键盘 Ctrl-C）。
pub const SIGINT: u32 = 2;
/// SIGILL：非法指令（#UD 等）。
pub const SIGILL: u32 = 4;
/// SIGBUS：总线错误（预留）。
pub const SIGBUS: u32 = 7;
/// SIGFPE：算术/浮点异常（#DE 除零等）。
pub const SIGFPE: u32 = 8;
/// SIGKILL：强制终止（不可捕获、不可屏蔽）。
pub const SIGKILL: u32 = 9;
/// SIGUSR1：应用自定义信号 1。
pub const SIGUSR1: u32 = 10;
/// SIGSEGV：段错误（#PF / #GP / #SS / #NP / #AC）。
pub const SIGSEGV: u32 = 11;
/// SIGUSR2：应用自定义信号 2。
pub const SIGUSR2: u32 = 12;
/// SIGPIPE：写已关闭管道。
pub const SIGPIPE: u32 = 13;
/// SIGALRM：定时器到期。
pub const SIGALRM: u32 = 14;
/// SIGTERM：兜底终止。
pub const SIGTERM: u32 = 15;
/// SIGCHLD：子进程状态变更。
pub const SIGCHLD: u32 = 17;
/// SIGCONT：恢复暂停的进程（预留）。
pub const SIGCONT: u32 = 18;
/// SIGSTOP：暂停进程（预留，首期 NotSupported）。
pub const SIGSTOP: u32 = 19;

// ---------- sigaction 处置哨兵（ADR-034 §3.3） ----------

/// SIG_DFL：默认处置（handler 参数传 0）。
pub const SIG_DFL: u64 = 0;
/// SIG_IGN：忽略该信号（handler 参数传 1）。
pub const SIG_IGN: u64 = 1;

// ---------- sigprocmask how（ADR-034 §3.3） ----------

/// SIGNAL_SET：blocked = set。
pub const SIGNAL_SET: u32 = 0;
/// SIGNAL_BLOCK：blocked |= set。
pub const SIGNAL_BLOCK: u32 = 1;
/// SIGNAL_UNBLOCK：blocked &= !set。
pub const SIGNAL_UNBLOCK: u32 = 2;

/// `action(sig, handler, flags) -> 旧处置`：查/设每信号处置（sigaction）。
///
/// `handler`：`SIG_DFL(0)` 设默认、`SIG_IGN(1)` 设忽略、其余为用户态 handler
/// 函数指针。返回**旧处置**（同样编码：0=Default、1=Ignore、其它为旧 handler
/// 指针）。对 SIGKILL/SIGSTOP 设非默认 → `Error::InvalidParam`。
pub fn action(sig: u32, handler: u64, flags: u32) -> Result<u64, Error> {
    crate::syscall::call(SYS_SIGNAL_ACTION, [sig as u64, handler, flags as u64, 0, 0, 0])
}

/// `mask(how, set) -> 旧屏蔽集`：查/改屏蔽集（sigprocmask）。
///
/// `how`：`SIGNAL_SET/BLOCK/UNBLOCK`；`set` 为屏蔽位图（bit63 须清零）。返回
/// **旧**屏蔽集。SIGKILL/SIGSTOP 恒强制置位不可解除。
pub fn mask(how: u32, set: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SIGNAL_MASK, [how as u64, set, 0, 0, 0, 0])
}

/// `pending() -> 待决集位图`：查询当前进程的**待决信号集**（POSIX `sigpending`）。
///
/// 与 [`mask`] 的屏蔽集是**不同**的集合：屏蔽集决定「哪些信号被挡住」，
/// 待决集是「挡住之后积压了哪些」。
pub fn pending() -> Result<u64, Error> {
    crate::syscall::call(SYS_SIGNAL_PENDING, [0, 0, 0, 0, 0, 0])
}

/// `suspend(mask)`：原子地把屏蔽集换成 `mask` 并挂起，直到有信号被投递（POSIX `sigsuspend`）。
///
/// 恒返回 `Err(Interrupted)`（POSIX：无论被哪个信号唤醒，`sigsuspend` 都返回 -1 置 EINTR）。
/// 屏蔽集由内核在**投递点**还原（见内核 `SYS_SIGNAL_SUSPEND` 的说明）。
pub fn suspend(mask: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SIGNAL_SUSPEND, [mask, 0, 0, 0, 0, 0])
}

/// `alarm(seconds)`：设置/取消一次性闹钟，返回**上一个**闹钟的剩余秒数（POSIX `alarm`）。
///
/// `seconds == 0` 取消。到期由内核投递 `SIGALRM`。
pub fn alarm(seconds: u32) -> Result<u64, Error> {
    crate::syscall::call(SYS_SIGNAL_ALARM, [seconds as u64, 0, 0, 0, 0, 0])
}

/// `altstack_set(base, size) -> (旧 base, 旧 size)`：设置备用信号栈（POSIX `sigaltstack`）。
///
/// `(0, 0)` = 禁用。返回旧值；未设置过则为 `(0, 0)`。
pub fn altstack_set(base: u64, size: u64) -> Result<(u64, u64), Error> {
    altstack_op(base, size, 0)
}

/// `altstack_get() -> (base, size)`：**纯查询**当前备用信号栈（不改状态）。
pub fn altstack_get() -> Result<(u64, u64), Error> {
    altstack_op(0, 0, 1)
}

/// `altstack_op(base, size, mode)`：`mode` 0 = 设置、1 = 纯查询（见内核契约）。
fn altstack_op(base: u64, size: u64, mode: u64) -> Result<(u64, u64), Error> {
    let mut old = [0u8; 16];
    crate::syscall::call(
        SYS_SIGNAL_ALTSTACK,
        [base, size, old.as_mut_ptr() as u64, mode, 0, 0],
    )?;
    let mut b = [0u8; 8];
    let mut s = [0u8; 8];
    b.copy_from_slice(&old[..8]);
    s.copy_from_slice(&old[8..]);
    Ok((u64::from_le_bytes(b), u64::from_le_bytes(s)))
}

/// `raise(pid, sig)`：向进程投递信号（复用 TASK 域 kill，ADR-034 §2.3）。
///
/// 与 `process::kill` 等价；此处提供信号语义命名。调用方须提供目标 pid
/// （自己取自身 pid 即"向自身发信号"）。
pub fn raise(pid: u64, sig: u32) -> Result<u64, Error> {
    crate::process::kill(pid, sig as u64)
}

/// 信号号 → 名称（打印用）。
pub fn name(sig: u32) -> &'static str {
    match sig {
        SIGINT => "SIGINT",
        SIGILL => "SIGILL",
        SIGBUS => "SIGBUS",
        SIGFPE => "SIGFPE",
        SIGKILL => "SIGKILL",
        SIGUSR1 => "SIGUSR1",
        SIGSEGV => "SIGSEGV",
        SIGUSR2 => "SIGUSR2",
        SIGPIPE => "SIGPIPE",
        SIGALRM => "SIGALRM",
        SIGTERM => "SIGTERM",
        SIGCHLD => "SIGCHLD",
        SIGCONT => "SIGCONT",
        SIGSTOP => "SIGSTOP",
        0 => "0",
        _ => "?",
    }
}

/// 已知信号列表（供 `kill -l` 列出）。
pub const LIST: &[(u32, &str)] = &[
    (SIGINT, "SIGINT"),
    (SIGILL, "SIGILL"),
    (SIGBUS, "SIGBUS"),
    (SIGFPE, "SIGFPE"),
    (SIGKILL, "SIGKILL"),
    (SIGUSR1, "SIGUSR1"),
    (SIGSEGV, "SIGSEGV"),
    (SIGUSR2, "SIGUSR2"),
    (SIGPIPE, "SIGPIPE"),
    (SIGALRM, "SIGALRM"),
    (SIGTERM, "SIGTERM"),
    (SIGCHLD, "SIGCHLD"),
    (SIGCONT, "SIGCONT"),
    (SIGSTOP, "SIGSTOP"),
];
