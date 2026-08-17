//! 进程（PROCESS 域）薄封装。

use crate::nr::SYS_EXIT;

/// `exit(code)`：终止当前进程。永不返回。
pub fn exit(code: i32) -> ! {
    let _ = crate::syscall::invoke(SYS_EXIT, code as u64, 0, 0, 0, 0, 0);
    // 内核 `exit` 不返回；此处兜底自旋（防御性，正常不可达）。
    loop {
        core::hint::spin_loop();
    }
}
