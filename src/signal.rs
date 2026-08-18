//! 信号常量——与内核 `kernel/src/signals.rs` 对齐。
//!
//! 雏形阶段仅导出信号号常量；内核在用户态异常（#PF/#UD/#GP 等）时把进程
//! 异常终止归类为对应信号。完整信号派发 / 用户 handler 回调留待 M5。

/// SIGILL：非法指令（#UD 等）。
pub const SIGILL: u32 = 4;
/// SIGBUS：总线错误（预留）。
pub const SIGBUS: u32 = 7;
/// SIGFPE：算术/浮点异常（#DE 除零等）。
pub const SIGFPE: u32 = 8;
/// SIGSEGV：段错误（#PF / #GP / #SS / #NP / #AC）。
pub const SIGSEGV: u32 = 11;
/// SIGTERM：兜底终止。
pub const SIGTERM: u32 = 15;
/// SIGKILL：强制终止（不可捕获）。
pub const SIGKILL: u32 = 9;

/// 信号号 → 名称（打印用）。
pub fn name(sig: u32) -> &'static str {
    match sig {
        SIGKILL => "SIGKILL",
        SIGTERM => "SIGTERM",
        0 => "0",
        _ => "?",
    }
}

/// 已知信号列表（供 `kill -l` 列出）。
pub const LIST: &[(u32, &str)] = &[(SIGKILL, "SIGKILL"), (SIGTERM, "SIGTERM")];
