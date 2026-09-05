//! 线程（同进程调度单元）薄封装（T1-7 / ADR-035 / threads.md T1-7）。
//!
//! 线程 = 组长进程（组长 = 调用方自身进程，其 tgid == pid）内另一个**同组调度单元**：
//! 共享 ThreadGroup（addr_space/fd/cwd/identity）。用户态以 `mmap` 自备新线程独立
//! 用户栈区，派发时传 `entry` + `user_stack_top`（PRE-6）。
//!
//! 与 `process` 模块（进程级 spawn/exit/waitpid）的关系：
//! - `thread_spawn` = 组内派生，走 `SYS_TASK_THREAD_SPAWN`（0x35）；
//! - `thread_join` = 组长对具体组员 pid 的 waitpid 收尸，走 `SYS_TASK_THREAD_JOIN`（0x36）；
//! - `thread_exit` = `process::exit` 别名（SYS_TASK_EXIT / 0x34）：内核
//!   `terminate_locked` 已按调用方身份分流——组长调 = 整个进程退出并 notify 父；
//!   组员调 = 仅该成员单体退出、留 zombie 供组长 join，不杀整组。故不新增 nr。

use crate::error::Error;
use crate::nr::{SYS_TASK_THREAD_JOIN, SYS_TASK_THREAD_SPAWN};

/// `thread_spawn(entry, user_stack_top) -> tid`：在调用方线程组（组长 = 调用方自身
/// 进程）内派生一个同组新线程并运行于 `entry`，使用用户态已 mmap 的独立栈
/// （栈顶 = `user_stack_top`）。返回新线程 tid（= 组员 pid）。
///
/// 内核以调用方自身进程的 tgid 为组长（组长/组员调用本函数都派生到同一组长组）。
pub fn thread_spawn(entry: u64, user_stack_top: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_THREAD_SPAWN, [entry, user_stack_top, 0, 0, 0, 0])
}

/// `thread_join(tid) -> code`：组长阻塞等待**具体组员线程** `tid` 退出并收尸，
/// 返回其退出码。等价组长对组员 pid 的 waitpid 单目标收尸（T1-3 join 交付）。
///
/// - 组员已退出（zombie）→ 立即收尸返回退出码；
/// - 组员仍在运行 → 阻塞当前组长直到其退出；
/// - `tid` 非本组组员 / 不存在 / 已收尸（仅组长能 join 其组员）→ `Err(NotFound)`。
/// 内核交付协议：rax=退出码、r10=被收尸组员 pid（与 waitpid 一致，经 aux_pid/saved.r10）。
pub fn thread_join(tid: u64) -> Result<u64, Error> {
    let (ret, _r10) = crate::syscall::invoke_capture_r10(SYS_TASK_THREAD_JOIN, tid, 0, 0, 0, 0, 0);
    if ret & (1u64 << 63) != 0 {
        return Err(crate::error::Error::from_errno((ret as i64).wrapping_neg() as i32));
    }
    Ok(ret)
}

/// `thread_exit(code) -> !`：终止当前执行体，永不返回。身份语义由内核判定：
/// 组长调用 = 整个进程退出（notify 自己的父）；组员调用 = 仅该成员单体退出（不杀整组）。
/// 复用 `SYS_TASK_EXIT`，等价 `process::exit`——不新增 syscall 号。
pub fn thread_exit(code: i32) -> ! {
    crate::process::exit(code)
}
