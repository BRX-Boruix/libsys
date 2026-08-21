//! IPC（M5）薄封装：遵循 ADR-014（统一走 MEMORY 与 STREAM 域）。

use crate::error::Error;
use crate::nr::{
    STREAM_OFFSET_CURRENT, SYS_MEMORY_MAP, SYS_MEMORY_UNMAP, SYS_STREAM_CLOSE, SYS_STREAM_CREATE,
    SYS_STREAM_READ, SYS_STREAM_WRITE,
};

/// `shm_create(size) -> id`：创建一块大小为 `size` 的共享内存对象（走 SYS_MEMORY_MAP）。
pub fn shm_create(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0x01 /* shared flag */, 0, 0, 0, 0])
}

/// `shm_map(id) -> addr`：把共享内存对象映射进本进程地址空间（走 SYS_MEMORY_MAP）。
pub fn shm_map(id: u64) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_MEMORY_MAP,
        [0, 0x02 /* map existing flag */, id, 0, 0, 0],
    )
}

/// `shm_unmap(id)`：解除本进程对该共享内存的映射（走 SYS_MEMORY_UNMAP）。
pub fn shm_unmap(id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_MEMORY_UNMAP, [id, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `pipe_create() -> id`：创建一条匿名管道（走 SYS_STREAM_CREATE(null, FLAG_PIPE)）。
pub fn pipe_create() -> Result<u64, Error> {
    crate::syscall::call(SYS_STREAM_CREATE, [0, 0x80 /* FLAG_PIPE */, 0, 0, 0, 0])
}

/// `pipe_write(id, buf) -> n`：把 `buf` 写入管道（走 SYS_STREAM_WRITE）。
pub fn pipe_write(id: u64, buf: &[u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_STREAM_WRITE,
        [
            id,
            buf.as_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            0,
            0,
        ],
    )
}

/// `pipe_read(id, buf) -> n`：从管道读入 `buf`（走 SYS_STREAM_READ）。
pub fn pipe_read(id: u64, buf: &mut [u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_STREAM_READ,
        [
            id,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            0,
            0,
        ],
    )
}

/// `pipe_close(id)`：销毁管道（走 SYS_STREAM_CLOSE）。
pub fn pipe_close(id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_STREAM_CLOSE, [id, 0, 0, 0, 0, 0]).map(|_| ())
}
