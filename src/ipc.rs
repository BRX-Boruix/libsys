//! IPC（M5）薄封装：共享内存 + 管道。

use crate::error::Error;
use crate::nr::{SYS_PIPE_CLOSE, SYS_PIPE_CREATE, SYS_PIPE_READ, SYS_PIPE_WRITE, SYS_SHM_CREATE, SYS_SHM_MAP, SYS_SHM_UNMAP};

/// `shm_create(size) -> id`：创建一块大小为 `size`（向上取整到页）的共享内存。
///
/// 返回共享内存对象 id，可被多个进程 `shm_map` 共享。
pub fn shm_create(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SHM_CREATE, [size, 0, 0, 0, 0, 0])
}

/// `shm_map(id) -> addr`：把共享内存对象映射进本进程地址空间，返回起始地址。
///
/// 多进程映射同一 id 共享同一批物理页：写一方可见于其它方。
pub fn shm_map(id: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SHM_MAP, [id, 0, 0, 0, 0, 0])
}

/// `shm_unmap(id)`：解除本进程对该共享内存的映射（最后一次时内核释放帧）。
pub fn shm_unmap(id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_SHM_UNMAP, [id, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `pipe_create() -> id`：创建一条管道，返回管道 id。
pub fn pipe_create() -> Result<u64, Error> {
    crate::syscall::call(SYS_PIPE_CREATE, [0, 0, 0, 0, 0, 0])
}

/// `pipe_write(id, buf) -> n`：把 `buf` 写入管道（缓冲满时阻塞），返回写入字节数。
pub fn pipe_write(id: u64, buf: &[u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_PIPE_WRITE,
        [id, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0],
    )
}

/// `pipe_read(id, buf) -> n`：从管道读入 `buf`（缓冲空时阻塞），返回读到的字节数。
pub fn pipe_read(id: u64, buf: &mut [u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_PIPE_READ,
        [id, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0],
    )
}

/// `pipe_close(id)`：销毁管道。
pub fn pipe_close(id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_PIPE_CLOSE, [id, 0, 0, 0, 0, 0]).map(|_| ())
}
