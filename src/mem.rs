//! 内存（MEMORY 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_BRK, SYS_MMAP};

/// `mmap(size)`：预留一段按需分页区，返回起始地址。
pub fn mmap(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MMAP, [size, 0, 0, 0, 0, 0])
}

/// `brk(new)`：调整堆断点（0 = 查询），返回新断点。
pub fn brk(new_break: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_BRK, [new_break, 0, 0, 0, 0, 0])
}
