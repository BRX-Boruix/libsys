//! 输入输出（IO 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_READ, SYS_WRITE};

/// 标准输入 / 标准输出 / 标准错误文件描述符。
pub const STDIN: u64 = 0;
pub const STDOUT: u64 = 1;
pub const STDERR: u64 = 2;

/// `read(fd, buf)`：从 fd 读字节到缓冲（0=stdin 键盘），返回实际读到的字节数。
/// 阻塞读：内核键盘输入缓冲空时等待，直到有数据或返回错误。
pub fn read(fd: u64, buf: &mut [u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_READ,
        [fd, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0],
    )
    .map(|n| n as usize)
}

/// `write(fd, buf)`：把字节缓冲写到 fd，返回写入字节数。
pub fn write(fd: u64, buf: &[u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_WRITE,
        [fd, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0],
    )
    .map(|n| n as usize)
}
