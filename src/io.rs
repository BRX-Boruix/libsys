//! 输入输出（IO 域）与 VFS 系统调用薄封装（ADR-011 / M6.2）。

use crate::error::Error;
use crate::nr::*;
use alloc::string::String;
use alloc::vec::Vec;

/// 标准输入 / 标准输出 / 标准错误文件描述符。
pub const STDIN: u64 = 0;
pub const STDOUT: u64 = 1;
pub const STDERR: u64 = 2;

/// 打开标志。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
    pub directory: bool,
}

impl OpenFlags {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        create: false,
        truncate: false,
        append: false,
        directory: false,
    };

    pub const WRITE_ONLY: Self = Self {
        read: false,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
    };

    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
    };

    pub const CREATE_OR_TRUNCATE: Self = Self {
        read: true,
        write: true,
        create: true,
        truncate: true,
        append: false,
        directory: false,
    };

    pub const fn to_bits(self) -> u32 {
        let mut bits = 0;
        if self.read {
            bits |= 1 << 0;
        }
        if self.write {
            bits |= 1 << 1;
        }
        if self.create {
            bits |= 1 << 2;
        }
        if self.truncate {
            bits |= 1 << 3;
        }
        if self.append {
            bits |= 1 << 4;
        }
        if self.directory {
            bits |= 1 << 5;
        }
        bits
    }
}

/// 权限能力标签。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub system_only: bool,
}

impl Permissions {
    pub const fn all() -> Self {
        Self {
            readable: true,
            writable: true,
            executable: true,
            system_only: false,
        }
    }

    pub const fn read_write() -> Self {
        Self {
            readable: true,
            writable: true,
            executable: false,
            system_only: false,
        }
    }

    pub const fn readonly() -> Self {
        Self {
            readable: true,
            writable: false,
            executable: false,
            system_only: false,
        }
    }

    pub const fn to_bits(self) -> u32 {
        let mut bits = 0;
        if self.readable {
            bits |= 1 << 0;
        }
        if self.writable {
            bits |= 1 << 1;
        }
        if self.executable {
            bits |= 1 << 2;
        }
        if self.system_only {
            bits |= 1 << 3;
        }
        bits
    }
}

/// 目录项条目。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub node_type: String,
    pub size: u64,
}

/// `open(path, flags, perm)`：打开或创建文件/流，返回句柄（fd）。
pub fn open(path: &str, flags: OpenFlags, perm: Permissions) -> Result<u64, Error> {
    if path.len() >= 256 {
        let mut null_terminated = Vec::with_capacity(path.len() + 1);
        null_terminated.extend_from_slice(path.as_bytes());
        null_terminated.push(0);
        return crate::syscall::call(
            SYS_STREAM_CREATE,
            [
                null_terminated.as_ptr() as u64,
                flags.to_bits() as u64,
                perm.to_bits() as u64,
                0,
                0,
                0,
            ],
        );
    }
    let mut buf = [0u8; 256];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;

    crate::syscall::call(
        SYS_STREAM_CREATE,
        [
            buf.as_ptr() as u64,
            flags.to_bits() as u64,
            perm.to_bits() as u64,
            0,
            0,
            0,
        ],
    )
}

/// `close(fd)`：关闭文件/流句柄。
pub fn close(fd: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_STREAM_CLOSE, [fd, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `read(fd, buf)`：从 fd 读字节到缓冲（流式自增读），返回实际读到的字节数。
pub fn read(fd: u64, buf: &mut [u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_READ,
        [
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            0,
            0,
        ],
    )
    .map(|n| n as usize)
}

/// `write(fd, buf)`：把字节缓冲写到 fd（流式自增写），返回写入字节数。
pub fn write(fd: u64, buf: &[u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_WRITE,
        [
            fd,
            buf.as_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            0,
            0,
        ],
    )
    .map(|n| n as usize)
}

/// `pread(fd, buf, offset)`：显式无状态定位读（统一走 SYS_STREAM_READ）。
pub fn pread(fd: u64, buf: &mut [u8], offset: u64) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_READ,
        [fd, buf.as_mut_ptr() as u64, buf.len() as u64, offset, 0, 0],
    )
    .map(|n| n as usize)
}

/// `pwrite(fd, buf, offset)`：显式无状态定位写（统一走 SYS_STREAM_WRITE）。
pub fn pwrite(fd: u64, buf: &[u8], offset: u64) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_WRITE,
        [fd, buf.as_ptr() as u64, buf.len() as u64, offset, 0, 0],
    )
    .map(|n| n as usize)
}

/// `mkdir(path, perm)`：创建目录。
pub fn mkdir(path: &str, perm: Permissions) -> Result<(), Error> {
    if path.len() >= 256 {
        let mut null_terminated = Vec::with_capacity(path.len() + 1);
        null_terminated.extend_from_slice(path.as_bytes());
        null_terminated.push(0);
        return crate::syscall::call(
            SYS_ENTRY_CREATE,
            [
                null_terminated.as_ptr() as u64,
                crate::nr::ENTRY_KIND_DIRECTORY,
                perm.to_bits() as u64,
                0,
                0,
                0,
            ],
        )
        .map(|_| ());
    }
    let mut buf = [0u8; 256];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;

    crate::syscall::call(
        SYS_ENTRY_CREATE,
        [
            buf.as_ptr() as u64,
            crate::nr::ENTRY_KIND_DIRECTORY,
            perm.to_bits() as u64,
            0,
            0,
            0,
        ],
    )
    .map(|_| ())
}

/// `unlink(path)`：删除文件或目录。
pub fn unlink(path: &str) -> Result<(), Error> {
    if path.len() >= 256 {
        let mut null_terminated = Vec::with_capacity(path.len() + 1);
        null_terminated.extend_from_slice(path.as_bytes());
        null_terminated.push(0);
        return crate::syscall::call(
            SYS_ENTRY_DELETE,
            [null_terminated.as_ptr() as u64, 0, 0, 0, 0, 0],
        )
        .map(|_| ());
    }
    let mut buf = [0u8; 256];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;

    crate::syscall::call(SYS_ENTRY_DELETE, [buf.as_ptr() as u64, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `chdir(path)`：切换当前进程工作目录（VFS 域 0x45）。
///
/// 相对路径相对当前 cwd 解析（内核 syscall 层拼接，VFS 只接受绝对路径）。
pub fn chdir(path: &str) -> Result<(), Error> {
    if path.len() >= 256 {
        let mut null_terminated = Vec::with_capacity(path.len() + 1);
        null_terminated.extend_from_slice(path.as_bytes());
        null_terminated.push(0);
        return crate::syscall::call(
            SYS_ENTRY_CHDIR,
            [null_terminated.as_ptr() as u64, 0, 0, 0, 0, 0],
        )
        .map(|_| ());
    }
    let mut buf = [0u8; 256];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;

    crate::syscall::call(SYS_ENTRY_CHDIR, [buf.as_ptr() as u64, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `getcwd()`：读当前进程工作目录（VFS 域 0x46），返回以 `/` 开头的绝对路径。
pub fn getcwd() -> Result<alloc::string::String, Error> {
    let mut buf = [0u8; 256];
    let n = crate::syscall::call(SYS_ENTRY_GETCWD, [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0])?;
    let n = n as usize;
    if n >= buf.len() {
        return Err(Error::OutOfRange);
    }
    let s = core::str::from_utf8(&buf[..n]).map_err(|_| Error::InvalidParam)?;
    Ok(alloc::string::String::from(s))
}

/// 高阶便捷函数：读取文件全部内容到 `Vec<u8>`。
pub fn read_to_end(path: &str) -> Result<Vec<u8>, Error> {
    let fd = open(path, OpenFlags::READ_ONLY, Permissions::readonly())?;
    let mut data = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match read(fd, &mut chunk) {
            Ok(0) => break,
            Ok(n) => data.extend_from_slice(&chunk[..n]),
            Err(e) => {
                let _ = close(fd);
                return Err(e);
            }
        }
    }
    close(fd)?;
    Ok(data)
}

/// 高阶便捷函数：读取目录下的所有目录项（原生从 VFS JSON 解析）。
pub fn read_dir(path: &str) -> Result<Vec<DirEntry>, Error> {
    let mut null_terminated = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    null_terminated[..path.len()].copy_from_slice(path.as_bytes());
    null_terminated[path.len()] = 0;

    let mut buf = [0u8; 2048];
    let n = crate::syscall::call(
        SYS_ENTRY_READ,
        [
            null_terminated.as_ptr() as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
            0,
            0,
        ],
    )? as usize;

    let text = core::str::from_utf8(&buf[..n]).map_err(|_| Error::InvalidParam)?;
    let mut entries = Vec::new();

    // 如果是 JSON 数组 [{"name":"...","type":"...","size":...}]
    let trimmed = text.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let content = &trimmed[1..trimmed.len() - 1];
        for obj_str in content.split("},") {
            let s = obj_str.trim().trim_start_matches('{').trim_end_matches('}');
            let mut name = String::new();
            let mut node_type = String::new();
            let mut size = 0u64;

            for field in s.split(',') {
                let mut kv = field.split(':');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    let k = k.trim().trim_matches('"');
                    let v = v.trim().trim_matches('"');
                    match k {
                        "name" => name = String::from(v),
                        "type" => node_type = String::from(v),
                        "size" => size = v.parse::<u64>().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            if !name.is_empty() {
                entries.push(DirEntry {
                    name,
                    node_type,
                    size,
                });
            }
        }
        return Ok(entries);
    }

    // 纯文本兼容
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            let size = parts[2].parse::<u64>().unwrap_or(0);
            entries.push(DirEntry {
                name: String::from(parts[0]),
                node_type: String::from(parts[1]),
                size,
            });
        }
    }
    Ok(entries)
}
