//! 用户态错误码——与内核 `klib::error::Error`（ADR-010）对齐。
//!
//! 内核 syscall 返回错误时置 `bit63`（等价 `-errno` 补码）。libsys 解包后
//! 映射回 Rust 风格 `Error`，供 `Result<T, Error>` 使用（ADR-003）。

use core::fmt;

/// 用户态错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// 内存耗尽。
    OutOfMemory,
    /// 参数非法。
    InvalidParam,
    /// 数值越界。
    OutOfRange,
    /// 目标不存在。
    NotFound,
    /// 目标已存在。
    AlreadyExists,
    /// 不支持的操作。
    NotSupported,
    /// 非阻塞操作无法立即完成。
    WouldBlock,
    /// 空间不足。
    NoSpace,
    /// 设备 I/O 错误。
    Io,
    /// 未知错误码（内核返回了 libsys 未识别的 errno）。
    Unknown(i32),
}

impl Error {
    /// 从 errno 数值映射回错误码（与内核 `Error::to_errno` 互逆）。
    pub fn from_errno(e: i32) -> Self {
        match e {
            12 => Error::OutOfMemory,   // ENOMEM
            22 => Error::InvalidParam,  // EINVAL
            34 => Error::OutOfRange,    // ERANGE
            2 => Error::NotFound,       // ENOENT
            17 => Error::AlreadyExists, // EEXIST
            95 => Error::NotSupported,  // ENOTSUP
            11 => Error::WouldBlock,    // EAGAIN
            28 => Error::NoSpace,       // ENOSPC
            5 => Error::Io,             // EIO
            other => Error::Unknown(other),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfMemory => f.write_str("out of memory"),
            Error::InvalidParam => f.write_str("invalid parameter"),
            Error::OutOfRange => f.write_str("out of range"),
            Error::NotFound => f.write_str("not found"),
            Error::AlreadyExists => f.write_str("already exists"),
            Error::NotSupported => f.write_str("not supported"),
            Error::WouldBlock => f.write_str("would block"),
            Error::NoSpace => f.write_str("no space"),
            Error::Io => f.write_str("i/o error"),
            Error::Unknown(_e) => f.write_str("unknown error"),
        }
    }
}
