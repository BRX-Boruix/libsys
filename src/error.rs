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
    /// 阻塞中的系统调用被信号打断（EINTR，ADR-051）。
    Interrupted,
    /// 空间不足。
    NoSpace,
    /// 设备 I/O 错误。
    Io,
    /// 目标不是目录。
    NotDirectory,
    /// 目标是目录（例如试图把目录当可执行文件装载）。
    IsDirectory,
    /// 权限不足。
    PermissionDenied,
    /// 用户态地址非法。
    BadAddress,
    /// 目录非空。
    NotEmpty,
    /// 名称过长。
    NameTooLong,
    /// 参数列表过长（命令行超出内核上限）。
    ArgListTooLong,
    /// 符号链接层数过多。
    TooManySymlinks,
    /// 对不可寻址对象做 seek。
    IllegalSeek,
    /// **可执行格式非法**：文件读到了，但内容不是可装载的 ELF。
    ///
    /// 与 [`Error::NotSupported`] 的分界（内核 LD2）：ExecFormat 表达
    /// 「这份镜像本身不合法」，NotSupported 表达「这个操作不被支持」。
    /// 用户态据此可区分「文件坏了」与「内核不支持」。
    ExecFormat,
    /// 只读文件系统。
    ReadOnly,
    /// 文件系统结构损坏（数据已完整读到但内容非法，重试无意义）。
    Corrupt,
    /// 资源忙：目标被某项活动状态占用，**重试不会自愈**。
    Busy,
    /// 未知错误码（内核返回了 libsys 未识别的 errno）。
    Unknown(i32),
}

impl Error {
    /// 映射到 errno 数值（与内核 `Error::to_errno` 对齐，ADR-010）。
    pub fn to_errno(self) -> i32 {
        // 数值的唯一事实源：tools/abi/constants.json（本函数只做委托）。
        crate::abi_generated::to_errno(self)
    }

    /// 从 errno 数值映射回错误码（与内核 `Error::to_errno` 互逆）。
    pub fn from_errno(e: i32) -> Self {
        // 双向映射与 to_errno 同源（生成物保证互逆）。
        crate::abi_generated::from_errno(e)
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
        Error::Interrupted => f.write_str("interrupted"),
            Error::NoSpace => f.write_str("no space"),
            Error::Io => f.write_str("i/o error"),
            Error::NotDirectory => f.write_str("not a directory"),
            Error::IsDirectory => f.write_str("is a directory"),
            Error::PermissionDenied => f.write_str("permission denied"),
            Error::BadAddress => f.write_str("bad address"),
            Error::NotEmpty => f.write_str("directory not empty"),
            Error::NameTooLong => f.write_str("name too long"),
            Error::ArgListTooLong => f.write_str("argument list too long"),
            Error::TooManySymlinks => f.write_str("too many symbolic links"),
            Error::IllegalSeek => f.write_str("illegal seek"),
            Error::ExecFormat => f.write_str("exec format error"),
            Error::ReadOnly => f.write_str("read-only file system"),
            Error::Corrupt => f.write_str("filesystem structure corrupt"),
            Error::Busy => f.write_str("resource busy"),
            Error::Unknown(_e) => f.write_str("unknown error"),
        }
    }
}
