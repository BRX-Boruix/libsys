//! 内核同步字（通用 futex 等待/唤醒）用户态封装（ADR-032 ACCEPTED / SYNC 0x70 域）。
//!
//! 提供跨进程同步字对象的创建/等待/唤醒/销毁：`sync_create` 建一个内核同步字，
//! `sync_wait` 在值等于 `expected` 时阻塞直到值被改或超时，`sync_wake` 设新值并
//! 唤醒至多 `n` 个等待者，`sync_delete` 销毁。
//!
//! **值域约束（ABI，S09 如实）**：同步字值被限定为 bit63 清零的非负 u64——因为
//! `sync_wait` 经 `rax` 返回当前值，而 syscall 错误编码用 `bit63` 置位（`-errno`）。
//! 若允许任意 u64 值，bit63 置位的值会被误读为错误。互斥/条件变量/信号量等典型
//! 用法值恒为小非负整数（0/1），约束恒成立。调用方不得写入 bit63 置位的值。
//!
//! **阻塞语义**：`sync_wait` 阻塞时经调度器真实切走（`Switched`），内核把保存帧
//! rax 预置为唤醒时当前值（`sync_wake` 唤醒）或 `expected`（超时）。`timeout_ns = 0`
//! 为**阻塞至被唤醒**（无超时）；若值已 `!= expected` 则立即返回当前值（非阻塞）。

use crate::error::Error;
use crate::nr::{SYS_SYNC_CREATE, SYS_SYNC_DELETE, SYS_SYNC_WAIT, SYS_SYNC_WAKE};

/// `sync_create(init_value) -> sync_id`：创建一个内核同步字对象，初值 `init_value`。
///
/// 返回可跨进程传递的 `sync_id`（经共享内存/事件等自定义通道显式传给其他进程）。
pub fn sync_create(init_value: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SYNC_CREATE, [init_value, 0, 0, 0, 0, 0])
}

/// `sync_wait(sync_id, expected, timeout_ns) -> 当前值`：阻塞等待同步字变为
/// `!= expected`，或立即返回当前值（值已满足）。
///
/// - 若当前值 `!= expected`：立即返回当前值（非阻塞）。
/// - 若 `== expected` 且 `timeout_ns == 0`：阻塞至被 `sync_wake` 唤醒，返回新值。
/// - 若 `== expected` 且 `timeout_ns > 0`：阻塞至被唤醒或超时；超时返回 `expected`
///   （调用方见返回值 `== expected` 即知超时，可决定重试）。
///
/// 值域约束：返回值/`expected` 均须 bit63 清零（见模块头）。
pub fn sync_wait(sync_id: u64, expected: u64, timeout_ns: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SYNC_WAIT, [sync_id, expected, timeout_ns, 0, 0, 0])
}

/// `sync_wake(sync_id, value, n) -> 实际唤醒数`：设同步字为 `value`，唤醒至多 `n` 个
/// 在此字上阻塞的进程，返回实际唤醒数。`n == 0` → 仅改值不唤醒。
///
/// 被唤醒进程的 `sync_wait` 返回 `value`（值域约束：`value` 须 bit63 清零）。
pub fn sync_wake(sync_id: u64, value: u64, n: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_SYNC_WAKE, [sync_id, value, n, 0, 0, 0])
}

/// `sync_delete(sync_id)`：销毁同步字对象。仍有阻塞等待者时返回 `Error::Busy`。
pub fn sync_delete(sync_id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_SYNC_DELETE, [sync_id, 0, 0, 0, 0, 0]).map(|_| ())
}

