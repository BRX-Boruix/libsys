//! 时间（TIME 域）封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::SYS_TASK_WAIT;

/// 墙钟时间（真实年月日时分秒），来自 CMOS/BIOS 硬件时钟。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallClock {
    /// 年（如 2026）。
    pub year: u64,
    /// 月（1~12）。
    pub month: u64,
    /// 日（1~31）。
    pub day: u64,
    /// 时（0~23）。
    pub hour: u64,
    /// 分（0~59）。
    pub minute: u64,
    /// 秒（0~59）。
    pub second: u64,
}

/// `now()`：单调时钟，纳秒（现代 x86_64 直接走 RDTSC / 0 系统调用开销）。
pub fn now() -> u64 {
    let tsc: u64;
    unsafe {
        core::arch::asm!(
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") tsc,
            out("rdx") _,
            options(nomem, nostack)
        );
    }
    // 假设 1GHz (1 tick ≈ 1 ns)，提供纳秒单调时钟
    tsc
}

/// `read_wall_clock()`：读取当前墙钟时间（真实年月日时分秒）。
///
/// 从 SysFS `/system/info/time` 读取并解析 JSON。底层由内核
/// `arch_x86_64::rtc::read_time` 直读 CMOS/BIOS 硬件时钟（非单调时钟，
/// 反映真实日历时间）。JSON 字段缺失/类型不符时按 0 处理（宁缺毋假，
/// 调用方据 `Result` 判断整体成功与否）。
pub fn read_wall_clock() -> Result<WallClock, Error> {
    let data = crate::io::read_to_end("/system/info/time")?;
    let text = core::str::from_utf8(&data).map_err(|_| Error::InvalidParam)?;
    let parsed = crate::json::JsonParser::new(text).parse();
    let crate::json::JsonValue::Object(fields) = parsed.map_err(|_| Error::InvalidParam)? else {
        return Err(Error::InvalidParam);
    };
    let mut wc = WallClock {
        year: 0,
        month: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
    };
    for (k, v) in fields {
        let Some(val) = (match v {
            crate::json::JsonValue::Number(n) => n.parse::<u64>().ok(),
            _ => None,
        }) else {
            continue;
        };
        match k.as_str() {
            "year" => wc.year = val,
            "month" => wc.month = val,
            "day" => wc.day = val,
            "hour" => wc.hour = val,
            "minute" => wc.minute = val,
            "second" => wc.second = val,
            _ => {}
        }
    }
    Ok(wc)
}

/// `sleep(ns)`：睡眠指定纳秒（走 SYS_TASK_WAIT(0, ns)）。
pub fn sleep(ns: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_TASK_WAIT, [0, ns, 0, 0, 0, 0]).map(|_| ())
}
