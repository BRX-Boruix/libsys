//! 匿名管道（pipe）用户态封装（ADR-014 §4.1 FLAG_PIPE）。
//!
//! 内核侧 `SYS_STREAM_CREATE` 的 FLAG_PIPE 分支已接线：空路径 + `pipe` 标志位
//! 分配一对匿名管道流句柄，返回 `(read_fd << 32) | write_fd`（两 fd 均 < 2^32，
//! bit63 恒 0 = 成功）。本模块提供用户态入口 [`pipe_create`]，把打包值解包为
//! `(read_fd, write_fd)`。
//!
//! **管道 fd 读写复用既有 `read`/`write` syscall**——内核已按 `OpenHandle::Pipe`
//! 把 `SYS_STREAM_READ/WRITE` 路由到 `ipc::pipe_read/pipe_write`（环形缓冲 +
//! 内部阻塞/唤醒），用户态无需新增 read/write 封装（pipe-features.md 第 1 层）。

use crate::error::Error;
use crate::nr::SYS_STREAM_CREATE;

/// `pipe_create() -> (read_fd, write_fd)`：创建一对匿名管道端。
///
/// 返回解包后的 `(读端 fd, 写端 fd)`。两 fd 都引用同一 ipc 管道（环形缓冲）：
/// 向 `write_fd` 写、从 `read_fd` 读，内核按管道语义路由（阻塞/唤醒）。读写端
/// 用毕各自 `close`。
///
/// 阻塞/缓冲语义属内核 ipc 层（`ipc::pipe_*`）：写满阻塞、读空阻塞，均经调度
/// 器真实切换（不忙转）。用户态拿到的就是普通 fd，可进 `read`/`write`。
pub fn pipe_create() -> Result<(u64, u64), Error> {
    // 空路径 + pipe 标志位 → 内核 sys_open_pipe 分配一对匿名管道端。
    // path 传指针 0（空串等价：sys_open_pipe 接受空串 ""）。为避免歧义，
    // 这里显式传一个指向 NUL 的栈指针，而非 0（与 open() 的空路径语义一致）。
    let empty: [u8; 1] = [0];
    let packed = crate::syscall::call(
        SYS_STREAM_CREATE,
        [
            empty.as_ptr() as u64,
            crate::io::OpenFlags::pipe_only().to_bits() as u64,
            0,
            0,
            0,
            0,
        ],
    )?;
    // 成功时 bit63 恒 0（两 fd 均 < 2^32），故 packed 必然 < 2^63 = 非错误。
    let read_fd = (packed >> 32) as u32 as u64;
    let write_fd = (packed & 0xFFFF_FFFF) as u64;
    Ok((read_fd, write_fd))
}
