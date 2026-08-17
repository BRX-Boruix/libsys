//! 输入输出（IO 域）薄封装。

use crate::error::Error;
use crate::nr::SYS_WRITE;

/// 标准输出 / 标准错误文件描述符。
pub const STDOUT: u64 = 1;
pub const STDERR: u64 = 2;

/// `write(fd, buf)`：把字节缓冲写到 fd，返回写入字节数。
pub fn write(fd: u64, buf: &[u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_WRITE,
        [fd, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0],
    )
    .map(|n| n as usize)
}
