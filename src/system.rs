//! 系统（SYSTEM 域）薄封装。

use crate::error::Error;
use crate::nr::SYS_INFO;

/// `info(what)`：查询内核信息（版本号 / 启动时长 / CPU 数等）。
pub fn info(what: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_INFO, [what, 0, 0, 0, 0, 0])
}
