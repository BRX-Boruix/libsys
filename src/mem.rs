//! 内存（MEMORY 域）薄封装。

use crate::error::Error;
use crate::nr::{
    MEM_MAP_SHARED, SYS_MEMORY_GROW, SYS_MEMORY_MAP, SYS_MEMORY_PROTECT, SYS_MEMORY_UNMAP,
};

/// mmap 的 prot 位（3P4-4）。**与内核 mm::user_space 的编号同一事实**（两侧互指）。
pub const PROT_READ: u64 = 1 << 0;
pub const PROT_WRITE: u64 = 1 << 1;
pub const PROT_EXEC: u64 = 1 << 2;
/// prot == 0 的历史语义 = RW（早期 wire 未用该参数）。
pub const PROT_DEFAULT: u64 = PROT_READ | PROT_WRITE;

/// `mmap(size)`：预留一段按需分页区（默认权限 RW），返回起始地址。
pub fn mmap(size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, 0, 0, 0, 0])
}

/// `mmap_prot(size, prot)`：带权限的匿名映射（3P4-4）。
///
/// W^X 由内核**单点**拒绝（写+执行同页 → InvalidParam）；本层不预检——让拒绝来自权威处，
/// 避免策略在两处漂移（S13）。
pub fn mmap_prot(size: u64, prot: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, 0, prot, 0, 0])
}

/// `mprotect(addr, len, prot)`：修改已映射内存权限（MEMORY 域 0x25，3P4-5）。
///
/// W^X 由内核**单点**拒绝（写+执行同页 → InvalidParam），本层不预检（S13）。
/// `prot == 0`（POSIX PROT_NONE）当前如实 NotSupported——抽象层没有「存在但不可访问」
/// 的权限表示，内核拒绝而非假装成只读。
pub fn mprotect(addr: u64, len: u64, prot: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_MEMORY_PROTECT, [addr, len, prot, 0, 0, 0]).map(|_| ())
}

/// `shm_map(id, size) -> vaddr`：把既有共享内存对象映射进本进程（ADR-014
/// §4.2，旧 `SYS_SHM_MAP` 合并到 `memory_map` 的 `shared_id` 参数）。
///
/// `#[allow(dead_code)]`：作为 libsys 对外公开的共享内存 API（ADR-014 §4.2），
/// 当前尚无内部调用方，保留供未来用户程序使用（与 mmap/munmap/brk 同属
/// MEMORY 域薄封装）。
#[allow(dead_code)]
pub fn shm_map(id: u64, size: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_MEMORY_MAP, [size, 0, id, 0, 0, 0])
}

/// `shm_create(size) -> vaddr`：新建共享内存对象并映射（ADR-014 §4.2，旧
/// `SYS_SHM_CREATE` 合并到 `memory_map` 的 `MEM_MAP_SHARED` 标志），返回映射
/// 起始虚拟地址。
///
/// `#[allow(dead_code)]`：同 [`shm_map`]，对外公开的共享内存 API，暂无内部
/// 调用方，保留供未来使用。
#[allow(dead_code)]
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
