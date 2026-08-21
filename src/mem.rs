//! 内存（MEMORY 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_MEMORY_GROW, SYS_MEMORY_MAP, SYS_MEMORY_UNMAP};

/// `mmap(size)`：预留一段按需分页区，返回起始地址。
pub fn mmap(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, 0, 0, 0, 0])
}

/// `munmap(addr, size)`：释放 `mmap` 返回的匿名虚拟地址区间。
///
/// `addr` 与 `size` 必须均为 4KiB 的整数倍，且完整区间必须属于单个仍存在的
/// 匿名映射；零长度、跨区间、重复释放与非 mmap 区域均返回内核错误。
pub fn munmap(addr: u64, size: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_MEMORY_UNMAP, [addr, size, 0, 0, 0, 0]).map(|_| ())
}

/// `brk(new)`：调整堆断点（0 = 查询），返回新断点。
pub fn brk(new_break: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_GROW, [new_break, 0, 0, 0, 0, 0])
}
