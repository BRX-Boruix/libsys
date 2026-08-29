//! 随机数（`/devices/random`）薄封装。
//!
//! ADR-014：`SYS_RANDOM` 系统调用已删除，随机数统一经
//! `STREAM_READ("/devices/random")` 获取。本模块提供便捷封装。
//!
//! 诚实性：`/devices/random/status` 如实披露熵源与是否密码学安全；本封装只读
//! 主字符流，不承担熵源质量判断——需要密码学随机性的调用方应先查 `status`。

use crate::error::Error;
use crate::io::{OpenFlags, Permissions};

/// 从 `/devices/random` 读取 `buf.len()` 字节随机数据，返回实际读取的字节数。
///
/// 底层是只读字符流（无 EOF），每次 `read` 返回新随机字节。失败（如内核熵
/// 源不可用）返回 `Error`。
pub fn bytes(buf: &mut [u8]) -> Result<usize, Error> {
    if buf.is_empty() {
        return Ok(0);
    }
    let fd = crate::io::open("/devices/random", OpenFlags::READ_ONLY, Permissions::readonly())?;
    let n = crate::io::read(fd, buf)?;
    let _ = crate::io::close(fd);
    Ok(n)
}
