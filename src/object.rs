//! ADR-014-12 面向对象风格 API 包装（`Stream`/`Task`/`Vfs`）。
//!
//! 底层仍是 [`crate::io`] / [`crate::process`] / [`crate::system`] 的扁平
//! 函数，本模块提供结构体方法语法糖，使 4x4 正交系统调用矩阵在用户态
//! 代码中呈现为流畅的现代 Rust 风格。

use crate::error::Error;
use crate::io::{DirEntry, OpenFlags, Permissions};

// ---------- Stream ----------

/// 统一流式 I/O 资源（常规文件、终端设备、管道）。
pub struct Stream;

impl Stream {
    /// 打开或创建文件/流，返回句柄。
    pub fn open(path: &str, flags: OpenFlags) -> Result<u64, Error> {
        crate::io::open(path, flags, Permissions::read_write())
    }

    /// 从流句柄读取字节到缓冲区。
    pub fn read(fd: u64, buf: &mut [u8]) -> Result<usize, Error> {
        crate::io::read(fd, buf)
    }

    /// 向流句柄写入字节。
    pub fn write(fd: u64, buf: &[u8]) -> Result<usize, Error> {
        crate::io::write(fd, buf)
    }

    /// 关闭流句柄。
    pub fn close(fd: u64) -> Result<(), Error> {
        crate::io::close(fd).map(|_| ())
    }
}

// ---------- Task ----------

/// 执行体与调度单元（进程生命周期、协同调度、时钟休眠）。
pub struct Task;

impl Task {
    /// 加载可执行文件并以新进程执行，返回 pid。
    pub fn spawn(path: &str, args: &[&str]) -> Result<u64, Error> {
        // 将 args 连接为空格分隔的字节串（与命令行 ABI 约定一致）。
        let mut cmd = alloc::vec::Vec::new();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                cmd.push(b' ');
            }
            cmd.extend_from_slice(arg.as_bytes());
        }
        crate::process::exec_path(path, &cmd)
    }

    /// 等待指定子任务退出（阻塞）。
    pub fn waitpid(target_pid: usize) -> Result<u64, Error> {
        // 当前仅支持等待任意子进程（WAIT_ANY 语义）；target_pid 精确匹配
        // 留待后续扩展。
        let _ = target_pid;
        crate::process::waitpid_any()
    }

    /// 向指定任务发送信号。
    pub fn signal(pid: usize, sig: u32) -> Result<(), Error> {
        crate::process::kill(pid as u64, sig as u64).map(|_| ())
    }

    /// 终止当前任务。
    pub fn exit(code: i32) -> ! {
        crate::process::exit(code)
    }

    /// 主动让出 CPU。
    pub fn yield_now() -> Result<(), Error> {
        crate::process::yield_now()
    }
}

// ---------- Vfs ----------

/// 命名空间与元数据（目录项、层级树操作）。
pub struct Vfs;

impl Vfs {
    /// 读取目录项，返回结构化条目列表。
    pub fn read_directory(path: &str) -> Result<alloc::vec::Vec<DirEntry>, Error> {
        crate::io::read_dir(path)
    }

    /// 创建目录。
    pub fn mkdir(path: &str) -> Result<(), Error> {
        crate::io::mkdir(path, Permissions::all())
    }

    /// 删除文件或空目录。
    pub fn unlink(path: &str) -> Result<(), Error> {
        crate::io::unlink(path)
    }
}