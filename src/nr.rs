//! 系统调用号定义——与内核 `kernel/src/syscall.rs` 对齐（ADR-003）。
//!
//! 编码 `(domain << 8) | op`：高字节资源域 + 低字节统一操作码。
//! 用户态通过 `int 0x80` 触发，参数走 `rax=nr` + `rdi/rsi/rdx/r10/r8/r9`。

/// `read(fd, buf, len) -> n`：从 fd 读字节到缓冲（0=stdin 键盘）。
pub const SYS_READ: u32 = 0x2001;
/// `write(fd, buf, len) -> n`：把缓冲写到 fd（1=stdout，2=stderr）。
pub const SYS_WRITE: u32 = 0x2002;
/// `mmap(size) -> addr`：在当前进程预留一段按需分页区。
pub const SYS_MMAP: u32 = 0x1000;
/// `brk(new) -> break`：调整/查询堆断点（0 = 查询）。
pub const SYS_BRK: u32 = 0x1005;
/// `exec(prog) -> pid`：加载内核嵌入的用户程序（如 shell）为新进程并运行。
pub const SYS_EXEC: u32 = 0x0000;
/// `exit(code) -> !`：终止当前进程。
pub const SYS_EXIT: u32 = 0x0003;
/// `yield() -> 0`：当前进程主动让出 CPU（切到下一个就绪进程）。
pub const SYS_YIELD: u32 = 0x0004;
/// `now() -> ns`：单调时钟（纳秒）。
pub const SYS_NOW: u32 = 0x3001;
/// `sleep(ns)`：忙等/挂起睡眠。
pub const SYS_SLEEP: u32 = 0x3002;
/// `info(what) -> u64`：查询内核信息。
pub const SYS_INFO: u32 = 0xF005;
/// `shm_create(size) -> id`：创建共享内存对象。
pub const SYS_SHM_CREATE: u32 = 0x6000;
/// `shm_unmap(id)`：解除当前进程共享内存映射。
pub const SYS_SHM_UNMAP: u32 = 0x6003;
/// `shm_map(id) -> addr`：映射共享内存对象到当前进程。
pub const SYS_SHM_MAP: u32 = 0x6005;
/// `pipe_create() -> id`：创建管道。
pub const SYS_PIPE_CREATE: u32 = 0x6100;
/// `pipe_read(id, buf, len) -> n`：阻塞读。
pub const SYS_PIPE_READ: u32 = 0x6101;
/// `pipe_write(id, buf, len) -> n`：阻塞写。
pub const SYS_PIPE_WRITE: u32 = 0x6102;
/// `pipe_close(id)`：销毁管道。
pub const SYS_PIPE_CLOSE: u32 = 0x6103;

/// `exec` 程序池索引：shell（PID 2）。
pub const PROG_SHELL: u64 = 1;

/// `info` 查询项：内核版本号。
pub const INFO_VERSION: u64 = 0;
/// `info` 查询项：启动以来毫秒数。
pub const INFO_BOOT_MS: u64 = 1;
/// `info` 查询项：CPU 数。
pub const INFO_CPU_COUNT: u64 = 2;
/// `ps(buf, cap) -> count`：枚举存活进程快照（每条 8 字节：pid:u32 + state:u8 + pad）。
pub const SYS_PS: u32 = 0xF010;
/// `kill(pid, sig) -> 0`：向进程发送信号（9=SIGKILL / 15=SIGTERM 终止；0=仅校验存在）。
pub const SYS_KILL: u32 = 0xF020;
