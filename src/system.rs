//! 系统（SYSTEM 域）信息封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::*;

/// `info(what)`：查询系统信息（优先从 VFS 获取或基础版本查询）。
pub fn info(what: u64) -> Result<u64, Error> {
    match what {
        INFO_VERSION => Ok(0x000100), // v0.1.0
        INFO_CPU_COUNT => {
            if let Ok(data) = crate::io::read_to_end("/system/cpu") {
                if let Ok(text) = core::str::from_utf8(&data) {
                    if let Some(pos) = text.find("\"cores\":") {
                        let rest = &text[pos + 8..];
                        let num_str: alloc::string::String =
                            rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(n) = num_str.parse::<u64>() {
                            return Ok(n);
                        }
                    }
                }
            }
            Ok(1)
        }
        INFO_BOOT_MS => {
            // 通过 TSC 估算开机毫秒数
            let ns = crate::time::now();
            Ok(ns / 1_000_000)
        }
        _ => Err(Error::NotSupported),
    }
}
