//! 进程（PROCESS 域）薄封装。

use crate::error::Error;
use crate::nr::{SYS_EXIT, SYS_YIELD};

/// `exit(code)`：终止当前进程。永不返回。
pub fn exit(code: i32) -> ! {
    let _ = crate::syscall::invoke(SYS_EXIT, code as u64, 0, 0, 0, 0, 0);
    // 内核 `exit` 不返回；此处兜底自旋（防御性，正常不可达）。
    loop {
        core::hint::spin_loop();
    }
}

/// `yield_now()`：当前进程主动让出 CPU（切到下一个就绪进程）。
///
/// 仅当前进程一个就绪时，内核不切换，立即返回 `Ok(())`；否则让出后待
/// 下次被调度时返回。
pub fn yield_now() -> Result<(), Error> {
    crate::syscall::call(SYS_YIELD, [0, 0, 0, 0, 0, 0]).map(|_| ())
}
