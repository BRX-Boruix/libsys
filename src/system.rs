//! 系统（SYSTEM 域）信息封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::*;

/// `info(what)`：查询系统信息（优先从 VFS 获取或基础版本查询）。
pub fn info(what: u64) -> Result<u64, Error> {
    match what {
        INFO_VERSION => Ok(0x000100), // v0.1.0
        INFO_CPU_COUNT => {
            if let Ok(data) = crate::io::read_to_end("/system/info/cpu") {
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
            // 从 SysFS 读取内核用正确时钟计算的开机毫秒数
            // （`/system/info/kernel` 的 `uptime_ms`，来自 klib::time::now_millis，
            // 基于 LAPIC 时钟）。不要用裸 RDTSC——`time::now` 假设 1GHz
            // （1 tick ≈ 1ns），实际 CPU 频率远高于 1GHz，会使 uptime 虚快。
            // 与 INFO_CPU_COUNT 的 VFS 读取模式一致（ADR-013 JSON 第一等公民）。
            if let Ok(data) = crate::io::read_to_end("/system/info/kernel") {
                if let Ok(text) = core::str::from_utf8(&data) {
                    if let Ok(crate::json::JsonValue::Object(fields)) =
                        crate::json::JsonParser::new(text).parse()
                    {
                        for (k, v) in fields {
                            if k == "uptime_ms" {
                                if let crate::json::JsonValue::Number(num) = v {
                                    if let Ok(ms) = num.parse::<u64>() {
                                        return Ok(ms);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // SysFS 不可用或解析失败：宁缺毋假（ADR-027），返回错误由调用方兜底。
            Err(Error::NotFound)
        }
        _ => Err(Error::NotSupported),
    }
}
