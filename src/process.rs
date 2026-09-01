//! 任务与进程（TASK 域）薄封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::{SYS_TASK_EXIT, SYS_TASK_SIGNAL, SYS_TASK_SPAWN, SYS_TASK_WAIT};

/// `exec(prog, cmd)`：加载程序（可为内建索引或路径）为新进程（PID 2 等）并运行，返回新进程 pid。
pub fn exec(prog: u64, cmd: &[u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_TASK_SPAWN,
        [prog, cmd.as_ptr() as u64, cmd.len() as u64, 0, 0, 0],
    )
}

/// `exec_path(path, cmd)`：直接从 VFS 路径（如 `/programs/shell.elf`）加载并运行新进程。
pub fn exec_path(path: &str, cmd: &[u8]) -> Result<u64, Error> {
    let mut null_terminated = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    null_terminated[..path.len()].copy_from_slice(path.as_bytes());
    null_terminated[path.len()] = 0;

    crate::syscall::call(
        SYS_TASK_SPAWN,
        [
            null_terminated.as_ptr() as u64,
            cmd.as_ptr() as u64,
            cmd.len() as u64,
            0,
            0,
            0,
        ],
    )
}

/// 进程快照条目。
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

/// `ps(buf) -> count`：从 `/processes/list` VFS 虚拟文件读取并解析存活进程快照。
pub fn ps(buf: &mut [PsEntry]) -> Result<usize, Error> {
    let data = crate::io::read_to_end("/processes/list")?;
    let text = core::str::from_utf8(&data).map_err(|_| Error::InvalidParam)?;
    let mut count = 0;

    // 解析 JSON 列表 [{"pid":1,"state":"Running",...}]
    let trimmed = text.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let content = &trimmed[1..trimmed.len() - 1];
        for obj_str in content.split("},") {
            if count >= buf.len() {
                break;
            }
            let s = obj_str.trim().trim_start_matches('{').trim_end_matches('}');
            let mut pid = 0u32;
            let mut state = 1u8; // Ready

            for field in s.split(',') {
                let mut kv = field.split(':');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    let k = k.trim().trim_matches('"');
                    let v = v.trim().trim_matches('"');
                    match k {
                        "pid" => pid = v.parse::<u32>().unwrap_or(0),
                        "state" => match v {
                            "Ready" => state = 1,
                            "Running" => state = 2,
                            "Blocked" => state = 3,
                            _ => state = 0,
                        },
                        _ => {}
                    }
                }
            }
            if pid > 0 {
                buf[count] = PsEntry {
                    pid,
                    state,
                    _pad: [0; 3],
                };
                count += 1;
            }
        }
    }
    Ok(count)
}

/// 动态获取当前所有存活进程的快照列表（自动扩容）。
pub fn ps_list() -> Result<alloc::vec::Vec<PsEntry>, Error> {
    let mut entries = alloc::vec![PsEntry { pid: 0, state: 0, _pad: [0; 3] }; 32];
    let count = ps(&mut entries)?;
    entries.truncate(count);
    Ok(entries)
}

/// `kill(pid, sig) -> 0`：向进程发送信号（统一走 SYS_TASK_SIGNAL）。
pub fn kill(pid: u64, sig: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_SIGNAL, [pid, sig, 0, 0, 0, 0])
}

/// `exit(code)`：终止当前进程。永不返回。
pub fn exit(code: i32) -> ! {
    let _ = crate::syscall::invoke(SYS_TASK_EXIT, code as u64, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}

/// `yield_now()`：当前进程主动让出 CPU（走 SYS_TASK_WAIT(0, 0)）。
pub fn yield_now() -> Result<(), Error> {
    crate::syscall::call(SYS_TASK_WAIT, [0, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `TASK_WAIT` 的 `target_pid` 哨兵值：等待任意子进程退出。
/// 与内核侧 `task::scheduler::WAIT_ANY` 同值（`usize::MAX` / `u64::MAX`）。
pub const WAIT_ANY: u64 = u64::MAX;

/// waitpid 收割结果：被收尸子进程的 pid 与其退出码（POSIX waitpid 返回 pid、
/// status 承载退出码的语义在 libc 层拆分，此处两者一并真实交付）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitResult {
    /// 被收尸的子进程 pid。
    pub pid: u64,
    /// 子进程退出码。
    pub code: u64,
}

/// `waitpid_any()`：等待任意直接子进程退出，返回被收尸子进程的 `(pid, code)`。
/// 阻塞当前进程直到任一子进程退出。无子进程时返回 `Err(NotFound)`。
/// 内核交付协议：rax=退出码、r10=pid（同步路径经 aux_pid、阻塞路径经
/// saved.r10，两条路径一致），故用 `invoke_capture_r10` 同时取回两者。
pub fn waitpid_any() -> Result<WaitResult, Error> {
    let (ret, r10) = crate::syscall::invoke_capture_r10(SYS_TASK_WAIT, WAIT_ANY, 0, 0, 0, 0, 0);
    if ret & (1u64 << 63) != 0 {
        return Err(crate::error::Error::from_errno((ret as i64).wrapping_neg() as i32));
    }
    Ok(WaitResult { pid: r10, code: ret })
}
