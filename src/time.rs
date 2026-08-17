//! 时间（TIME 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_NOW, SYS_SLEEP};

/// `now()`：单调时钟，纳秒。
pub fn now() -> u64 {
    crate::syscall::call(SYS_NOW, [0, 0, 0, 0, 0, 0]).unwrap_or(0)
}

/// `sleep(ns)`：睡眠指定纳秒。
pub fn sleep(ns: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_SLEEP, [ns, 0, 0, 0, 0, 0]).map(|_| ())
}
