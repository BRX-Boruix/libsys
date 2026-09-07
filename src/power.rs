//! POWER 域（0x90, ADR-036）：机器电源管理（关机 / 重启）薄封装。
//!
//! 两个操作都是**终结性**的：内核端成功后机器断电 / 复位，控制流永不返回。
//! 故本模块返回 `Result<(), Error>`：只有内核端因电源管理不可用而**拒绝**时
//! 才会 `Err` 返回（调用方继续在用户态运行）；成功路径不会返回。
//!
//! 遵循 ADR-014 对象-动词正交 + 双侧镜像（S13）纪律，与内核 `syscall.rs`
//! 的 POWER 域常量/语义同值、注释互指。

use crate::error::Error;
use crate::nr::*;

/// `power_off() -> !`：请求 ACPI 软关机（S5）。
/// 成功后机器断电、永不返回；电源管理不可用（无 S5 信息/PM1 控制块）时
/// 返回 `Err`，调用方可显式报告。
pub fn power_off() -> Result<(), Error> {
    crate::syscall::call(SYS_POWER_OFF, [0, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `power_reboot() -> !`：请求系统重启。
/// 成功后机器复位、永不返回；复位机制不可用时返回 `Err`。
pub fn reboot() -> Result<(), Error> {
    crate::syscall::call(SYS_POWER_REBOOT, [0, 0, 0, 0, 0, 0]).map(|_| ())
}
