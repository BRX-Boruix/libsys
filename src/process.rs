//! 进程（PROCESS 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_EXEC, SYS_EXIT, SYS_KILL, SYS_PS, SYS_YIELD};

/// `exec(prog, cmd)`：加载内核嵌入的用户程序（`prog` 为嵌入池索引，如
/// `SHELL`）为新进程（PID 2 等）并运行，返回新进程 pid。
///
/// `cmd` 为可选命令行字节串（如 `b"echo hi"`）：非空时新进程以 `argc=1 /
/// argv[0]=cmd` 启动（一次性命令模式），用于后台作业；为空（`&[]`）则按
/// `argc=0` 正常启动（进入 REPL）。
pub fn exec(prog: u64, cmd: &[u8]) -> Result<u64, Error> {
    crate::syscall::call(SYS_EXEC, [prog, cmd.as_ptr() as u64, cmd.len() as u64, 0, 0, 0])
}

/// 进程快照条目（与内核 `ps_snapshot` 布局一致：pid:u32 + state:u8 + pad）。
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct PsEntry {
    /// 进程 id。
    pub pid: u32,
    /// 状态：1=Ready 2=Running 3=Blocked。
    pub state: u8,
    /// 填充（保留）。
    pub _pad: [u8; 3],
}

/// `ps(buf) -> count`：枚举存活进程写入 `buf`，返回写入条目数。
pub fn ps(buf: &mut [PsEntry]) -> Result<usize, Error> {
    let n = crate::syscall::call(
        SYS_PS,
        [buf.as_mut_ptr() as u64, (buf.len() * 8) as u64, 0, 0, 0, 0],
    )?;
    Ok(n as usize)
}

/// `kill(pid, sig) -> 0`：向进程发送信号（`sig` 见 `crate::signal`）。
pub fn kill(pid: u64, sig: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_KILL, [pid, sig, 0, 0, 0, 0])
}

/// `exit(code)`：终止当前进程。永不返回。
pub fn exit(code: i32) -> ! {
    let _ = crate::syscall::invoke(SYS_EXIT, code as u64, 0, 0, 0, 0, 0);
    // 内核 `exit` 不返回；此处兜底自旋（防御性，正常不可达）。
    loop {
        core::hint::spin_loop();
    }
}

/// `yield_now()`：当前进程主动让出 CPU（切到下一个就绪进程）。
///
/// 仅当前进程一个就绪时，内核不切换，立即返回 `Ok(())`；否则让出后待
/// 下次被调度时返回。
pub fn yield_now() -> Result<(), Error> {
    crate::syscall::call(SYS_YIELD, [0, 0, 0, 0, 0, 0]).map(|_| ())
}
