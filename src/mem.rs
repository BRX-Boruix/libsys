//! 内存（MEMORY 域）薄封装。

use crate::error::Error;
use crate::nr::{MEM_MAP_SHARED, SYS_MEMORY_GROW, SYS_MEMORY_MAP, SYS_MEMORY_UNMAP};

/// `mmap(size)`：预留一段按需分页区，返回起始地址。
pub fn mmap(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, 0, 0, 0, 0])
}

/// `shm_map(id, size) -> vaddr`：把既有共享内存对象映射进本进程（ADR-014
/// §4.2，旧 `SYS_SHM_MAP` 合并到 `memory_map` 的 `shared_id` 参数）。
pub fn shm_map(id: u64, size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, id, 0, 0, 0])
}

/// `shm_create(size) -> vaddr`：新建共享内存对象并映射（ADR-014 §4.2，旧
/// `SYS_SHM_CREATE` 合并到 `memory_map` 的 `MEM_MAP_SHARED` 标志），返回映射
/// 起始虚拟地址。
pub fn shm_create(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, MEM_MAP_SHARED, 0, 0, 0, 0])
}

/// `munmap(addr, size)`：释放 `mmap` 返回的匿名或共享虚拟地址区间。
///
/// `addr` 与 `size` 必须均为 4KiB 的整数倍，且完整区间必须属于单个仍存在的
/// 匿名/共享映射；零长度、跨区间、重复释放与非 mmap 区域均返回内核错误。
pub fn munmap(addr: u64, size: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_MEMORY_UNMAP, [addr, size, 0, 0, 0, 0]).map(|_| ())
}

/// `brk(new)`：调整堆断点（0 = 查询），返回新断点。
pub fn brk(new_break: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_GROW, [new_break, 0, 0, 0, 0, 0])
}
