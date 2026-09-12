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
        match self {
            Error::OutOfMemory => 12,   // ENOMEM
            Error::InvalidParam => 22,  // EINVAL
            Error::OutOfRange => 34,    // ERANGE
            Error::NotFound => 2,       // ENOENT
            Error::AlreadyExists => 17, // EEXIST
            Error::NotSupported => 95,  // ENOTSUP
            Error::WouldBlock => 11,    // EAGAIN
            Error::NoSpace => 28,       // ENOSPC
            Error::Io => 5,             // EIO
            Error::NotDirectory => 20,  // ENOTDIR
            Error::IsDirectory => 21,   // EISDIR
            Error::PermissionDenied => 13, // EACCES
            Error::BadAddress => 14,    // EFAULT
            Error::NotEmpty => 39,      // ENOTEMPTY
            Error::NameTooLong => 36,   // ENAMETOOLONG
            Error::ArgListTooLong => 7, // E2BIG
            Error::TooManySymlinks => 40, // ELOOP
            Error::IllegalSeek => 29,   // ESPIPE
            Error::ExecFormat => 8,     // ENOEXEC
            Error::ReadOnly => 30,      // EROFS
            Error::Corrupt => 117,      // EUCLEAN
            Error::Busy => 16,          // EBUSY
            Error::Unknown(e) => e,
        }
    }

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
            20 => Error::NotDirectory,  // ENOTDIR
            21 => Error::IsDirectory,   // EISDIR
            13 => Error::PermissionDenied, // EACCES
            14 => Error::BadAddress,    // EFAULT
            39 => Error::NotEmpty,      // ENOTEMPTY
            36 => Error::NameTooLong,   // ENAMETOOLONG
            7 => Error::ArgListTooLong, // E2BIG
            40 => Error::TooManySymlinks, // ELOOP
            29 => Error::IllegalSeek,   // ESPIPE
            8 => Error::ExecFormat,     // ENOEXEC
            30 => Error::ReadOnly,      // EROFS
            117 => Error::Corrupt,      // EUCLEAN
            16 => Error::Busy,          // EBUSY
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
