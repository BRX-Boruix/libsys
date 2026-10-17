//! **自动生成，请勿手改。**
//!
//! 源：tools/abi/constants.json
//! 生成：python tools/abi/generate.py
//! 门禁：python tools/abi/generate.py --check（已登记进 tools/checks/run_all.py）
//!
//! 手改本文件会被门禁发现；要改数值请改 JSON 再重新生成。

// ---------- syscall 号 ----------

pub const SYS_AUDIO_ATTACH: u32 = crate::nr::domain::AUDIO | 0x01; // 0xA1
pub const SYS_AUDIO_COMMIT: u32 = crate::nr::domain::AUDIO | 0x04; // 0xA4
pub const SYS_AUDIO_DETACH: u32 = crate::nr::domain::AUDIO | 0x02; // 0xA2
pub const SYS_AUDIO_FETCH: u32 = crate::nr::domain::AUDIO | 0x03; // 0xA3
pub const SYS_DEVICE_PROBE: u32 = crate::nr::domain::DEVICE | 0x08; // 0x58
pub const SYS_DRIVER_CLAIM: u32 = crate::nr::domain::DEVICE | crate::nr::op::WRITE; // 0x53
pub const SYS_DRIVER_DMA_ALLOC: u32 = crate::nr::domain::DEVICE | 0x0A; // 0x5A
pub const SYS_DRIVER_DMA_FREE: u32 = crate::nr::domain::DEVICE | 0x0B; // 0x5B
pub const SYS_DRIVER_DMA_PHYS: u32 = crate::nr::domain::DEVICE | 0x0C; // 0x5C
pub const SYS_DRIVER_EVENT_NEXT: u32 = crate::nr::domain::DEVICE | crate::nr::op::EVENT; // 0x57
pub const SYS_DRIVER_IRQ_WAIT: u32 = crate::nr::domain::DEVICE | 0x09; // 0x59
pub const SYS_DRIVER_QUERY: u32 = crate::nr::domain::DEVICE | crate::nr::op::READ; // 0x52
pub const SYS_DRIVER_REGISTER: u32 = crate::nr::domain::DEVICE | crate::nr::op::CREATE; // 0x51
pub const SYS_DRIVER_UNREGISTER: u32 = crate::nr::domain::DEVICE | crate::nr::op::DELETE; // 0x54
pub const SYS_ENTRY_CHDIR: u32 = crate::nr::domain::VFS | 0x05; // 0x45
pub const SYS_ENTRY_CREATE: u32 = crate::nr::domain::VFS | crate::nr::op::CREATE; // 0x41
pub const SYS_ENTRY_DELETE: u32 = crate::nr::domain::VFS | crate::nr::op::DELETE; // 0x44
pub const SYS_ENTRY_GETCWD: u32 = crate::nr::domain::VFS | 0x06; // 0x46
pub const SYS_ENTRY_LINK: u32 = crate::nr::domain::VFS | 0x09; // 0x49
pub const SYS_ENTRY_READ: u32 = crate::nr::domain::VFS | crate::nr::op::READ; // 0x42
pub const SYS_ENTRY_READLINK: u32 = crate::nr::domain::VFS | 0x08; // 0x48
pub const SYS_ENTRY_SYMLINK: u32 = crate::nr::domain::VFS | 0x07; // 0x47
pub const SYS_ENTRY_UPDATE: u32 = crate::nr::domain::VFS | crate::nr::op::WRITE; // 0x43
pub const SYS_MEMORY_GROW: u32 = crate::nr::domain::MEMORY | crate::nr::op::WRITE; // 0x23
pub const SYS_MEMORY_MAP: u32 = crate::nr::domain::MEMORY | crate::nr::op::CREATE; // 0x21
pub const SYS_MEMORY_PROTECT: u32 = crate::nr::domain::MEMORY | 0x05; // 0x25
pub const SYS_MEMORY_QUERY: u32 = crate::nr::domain::MEMORY | crate::nr::op::READ; // 0x22
pub const SYS_MEMORY_UNMAP: u32 = crate::nr::domain::MEMORY | crate::nr::op::DELETE; // 0x24
pub const SYS_POWER_OFF: u32 = crate::nr::domain::POWER | 0x01; // 0x91
pub const SYS_POWER_REBOOT: u32 = crate::nr::domain::POWER | 0x02; // 0x92
pub const SYS_SIGNAL_ACTION: u32 = crate::nr::domain::SIGNAL | crate::nr::op::WRITE; // 0x83
pub const SYS_SIGNAL_ALARM: u32 = crate::nr::domain::SIGNAL | 0x07; // 0x87
pub const SYS_SIGNAL_ALTSTACK: u32 = crate::nr::domain::SIGNAL | 0x08; // 0x88
pub const SYS_SIGNAL_MASK: u32 = crate::nr::domain::SIGNAL | crate::nr::op::READ; // 0x82
pub const SYS_SIGNAL_PENDING: u32 = crate::nr::domain::SIGNAL | 0x05; // 0x85
pub const SYS_SIGNAL_RETURN: u32 = crate::nr::domain::SIGNAL | crate::nr::op::DELETE; // 0x84
pub const SYS_SIGNAL_SUSPEND: u32 = crate::nr::domain::SIGNAL | 0x06; // 0x86
pub const SYS_STREAM_CLOSE: u32 = crate::nr::domain::STREAM | crate::nr::op::DELETE; // 0x14
pub const SYS_STREAM_CREATE: u32 = crate::nr::domain::STREAM | crate::nr::op::CREATE; // 0x11
pub const SYS_STREAM_DUP: u32 = crate::nr::domain::STREAM | 0x05; // 0x15
pub const SYS_STREAM_FCHMOD: u32 = crate::nr::domain::STREAM | 0x0B; // 0x1B
pub const SYS_STREAM_FCHOWN: u32 = crate::nr::domain::STREAM | 0x0C; // 0x1C
pub const SYS_STREAM_FOCUS_SET: u32 = crate::nr::domain::STREAM | 0x08; // 0x18
pub const SYS_STREAM_FSTAT: u32 = crate::nr::domain::STREAM | 0x07; // 0x17
pub const SYS_STREAM_FSYNC: u32 = crate::nr::domain::STREAM | 0x0A; // 0x1A
pub const SYS_STREAM_FTRUNCATE: u32 = crate::nr::domain::STREAM | 0x09; // 0x19
pub const SYS_STREAM_LOCK: u32 = crate::nr::domain::STREAM | 0x06; // 0x16
pub const SYS_STREAM_READ: u32 = crate::nr::domain::STREAM | crate::nr::op::READ; // 0x12
pub const SYS_STREAM_SEEK: u32 = crate::nr::domain::STREAM | 0x0D; // 0x1D
pub const SYS_STREAM_FLAGS: u32 = crate::nr::domain::STREAM | 0x0E; // 0x1E
pub const SYS_STREAM_WRITE: u32 = crate::nr::domain::STREAM | crate::nr::op::WRITE; // 0x13
pub const SYS_SYNC_CREATE: u32 = crate::nr::domain::SYNC | crate::nr::op::CREATE; // 0x71
pub const SYS_SYNC_DELETE: u32 = crate::nr::domain::SYNC | crate::nr::op::DELETE; // 0x74
pub const SYS_SYNC_WAIT: u32 = crate::nr::domain::SYNC | crate::nr::op::READ; // 0x72
pub const SYS_SYNC_WAKE: u32 = crate::nr::domain::SYNC | crate::nr::op::WRITE; // 0x73
pub const SYS_TASK_DERIVE: u32 = crate::nr::domain::TASK | 0x0A; // 0x3A
pub const SYS_TASK_EXIT: u32 = crate::nr::domain::TASK | crate::nr::op::DELETE; // 0x34
pub const SYS_TASK_GETPID: u32 = crate::nr::domain::TASK | 0x09; // 0x39
pub const SYS_TASK_GETTID: u32 = crate::nr::domain::TASK | 0x08; // 0x38
pub const SYS_TASK_GROUPS_GET: u32 = crate::nr::domain::TASK | 0x0E; // 0x3E
pub const SYS_TASK_GROUPS_SET: u32 = crate::nr::domain::TASK | 0x0D; // 0x3D
pub const SYS_TASK_IDENTITY_QUERY: u32 = crate::nr::domain::TASK | 0x0B; // 0x3B
pub const SYS_TASK_IDENTITY_SET: u32 = crate::nr::domain::TASK | 0x0C; // 0x3C
pub const SYS_TASK_NICE: u32 = crate::nr::domain::TASK | 0x0F; // 0x3F
pub const SYS_TASK_SET_FS_BASE: u32 = crate::nr::domain::TASK | 0x07; // 0x37
pub const SYS_TASK_SIGNAL: u32 = crate::nr::domain::TASK | crate::nr::op::WRITE; // 0x33
pub const SYS_TASK_SPAWN: u32 = crate::nr::domain::TASK | crate::nr::op::CREATE; // 0x31
pub const SYS_TASK_THREAD_JOIN: u32 = crate::nr::domain::TASK | 0x06; // 0x36
pub const SYS_TASK_THREAD_SPAWN: u32 = crate::nr::domain::TASK | 0x05; // 0x35
pub const SYS_TASK_WAIT: u32 = crate::nr::domain::TASK | crate::nr::op::READ; // 0x32
pub const SYS_VOLUME_FORMAT: u32 = crate::nr::domain::VOLUME | crate::nr::op::FORMAT; // 0x65
pub const SYS_VOLUME_LIST: u32 = crate::nr::domain::VOLUME | crate::nr::op::READ; // 0x62
pub const SYS_VOLUME_MOUNT: u32 = crate::nr::domain::VOLUME | crate::nr::op::CREATE; // 0x61
pub const SYS_VOLUME_UNMOUNT: u32 = crate::nr::domain::VOLUME | crate::nr::op::UNMOUNT; // 0x66
pub const SYS_VOLUME_UPDATE: u32 = crate::nr::domain::VOLUME | crate::nr::op::WRITE; // 0x63

// ---------- Error <-> errno（双向） ----------

pub const fn to_errno(e: crate::error::Error) -> i32 {
    match e {
        crate::error::Error::OutOfMemory => 12, // ENOMEM
        crate::error::Error::InvalidParam => 22, // EINVAL
        crate::error::Error::OutOfRange => 34, // ERANGE
        crate::error::Error::NotFound => 2, // ENOENT
        crate::error::Error::AlreadyExists => 17, // EEXIST
        crate::error::Error::NotSupported => 95, // ENOTSUP
        crate::error::Error::WouldBlock => 11, // EAGAIN
        crate::error::Error::Interrupted => 4, // EINTR
        crate::error::Error::NoSpace => 28, // ENOSPC
        crate::error::Error::Io => 5, // EIO
        crate::error::Error::NotDirectory => 20, // ENOTDIR
        crate::error::Error::IsDirectory => 21, // EISDIR
        crate::error::Error::PermissionDenied => 13, // EACCES
        crate::error::Error::BadAddress => 14, // EFAULT
        crate::error::Error::NotEmpty => 39, // ENOTEMPTY
        crate::error::Error::NameTooLong => 36, // ENAMETOOLONG
        crate::error::Error::ArgListTooLong => 7, // E2BIG
        crate::error::Error::TooManySymlinks => 40, // ELOOP
        crate::error::Error::IllegalSeek => 29, // ESPIPE
        crate::error::Error::ExecFormat => 8, // ENOEXEC
        crate::error::Error::ReadOnly => 30, // EROFS
        crate::error::Error::Corrupt => 117, // EUCLEAN
        crate::error::Error::Busy => 16, // EBUSY
        crate::error::Error::BadFd => 9, // EBADF
        crate::error::Error::NoChild => 10, // ECHILD
        crate::error::Error::NotPermitted => 1, // EPERM
        crate::error::Error::Unknown(e) => e,
    }
}

pub const fn from_errno(e: i32) -> crate::error::Error {
    match e {
        12 => crate::error::Error::OutOfMemory, // ENOMEM
        22 => crate::error::Error::InvalidParam, // EINVAL
        34 => crate::error::Error::OutOfRange, // ERANGE
        2 => crate::error::Error::NotFound, // ENOENT
        17 => crate::error::Error::AlreadyExists, // EEXIST
        95 => crate::error::Error::NotSupported, // ENOTSUP
        11 => crate::error::Error::WouldBlock, // EAGAIN
        4 => crate::error::Error::Interrupted, // EINTR
        28 => crate::error::Error::NoSpace, // ENOSPC
        5 => crate::error::Error::Io, // EIO
        20 => crate::error::Error::NotDirectory, // ENOTDIR
        21 => crate::error::Error::IsDirectory, // EISDIR
        13 => crate::error::Error::PermissionDenied, // EACCES
        14 => crate::error::Error::BadAddress, // EFAULT
        39 => crate::error::Error::NotEmpty, // ENOTEMPTY
        36 => crate::error::Error::NameTooLong, // ENAMETOOLONG
        7 => crate::error::Error::ArgListTooLong, // E2BIG
        40 => crate::error::Error::TooManySymlinks, // ELOOP
        29 => crate::error::Error::IllegalSeek, // ESPIPE
        8 => crate::error::Error::ExecFormat, // ENOEXEC
        30 => crate::error::Error::ReadOnly, // EROFS
        117 => crate::error::Error::Corrupt, // EUCLEAN
        16 => crate::error::Error::Busy, // EBUSY
        9 => crate::error::Error::BadFd, // EBADF
        10 => crate::error::Error::NoChild, // ECHILD
        1 => crate::error::Error::NotPermitted, // EPERM
        other => crate::error::Error::Unknown(other),
    }
}

// ---------- 跨 crate 数值（同一数值只在这里写一次） ----------

pub const FLOCK_CMD_SHARED: u64 = 0;
pub const FLOCK_CMD_EXCLUSIVE: u64 = 1;
pub const FLOCK_CMD_UNLOCK: u64 = 2;

// ---------- 系统路径（与内核 devfs 同一事实源） ----------

pub const CONSOLE_PATH: &str = "/devices/console";
pub const CONSOLES_DIR_PREFIX: &str = "/devices/consoles/";

// ---------- 系统标识 token（**只有系统约定**；部署事实在运行期发现） ----------

pub const PROGRAM_DIR: &str = "/programs";
pub const THIRD_PARTY_SUBDIR: &str = "3p";
pub const DATA_VOLUME_LABEL: &str = "BORUIX_DATA";

// ---------- 跨边界 wire 常量（内核 <-> libsys） ----------

pub const ACE_PRINCIPAL_NAMED_GID: u32 = 2;
pub const ACE_PRINCIPAL_NAMED_UID: u32 = 1;
pub const ACE_PRINCIPAL_OTHER: u32 = 3;
pub const ACE_PRINCIPAL_OWNER: u32 = 0;
pub const ACE_WIRE_MAX: usize = 64;
pub const ACE_WIRE_SIZE: usize = 24;
pub const AT_EXECFN: u64 = 31;
pub const AT_NULL: u64 = 0;
pub const DERIVE_FLAGS_NONE: u64 = 0;
pub const ENTRY_KIND_DIRECTORY: u64 = 0;
pub const ENTRY_KIND_FILE: u64 = 1;
pub const ENTRY_READ_ACES: u64 = 2;
pub const ENTRY_READ_READDIR: u64 = 0;
pub const ENTRY_READ_STAT: u64 = 1;
pub const ENTRY_UPDATE_CHMOD: u64 = 1;
pub const ENTRY_UPDATE_CHOWN: u64 = 2;
pub const ENTRY_UPDATE_RENAME: u64 = 0;
pub const ENTRY_UPDATE_SET_ACES: u64 = 3;
pub const EVENT_FLAG_E0: u8 = 1 << 0;
pub const EVENT_FLAG_NO_TIME: u8 = 1 << 1;
pub const EVENT_KIND_KEY_DOWN: u8 = 1;
pub const EVENT_KIND_KEY_UP: u8 = 2;
pub const EVENT_RECORD_SIZE: usize = 16;
pub const GATE_SYSTEM_BIT: u32 = 1 << 9;
pub const GROUPS_SET_CLEAR: u64 = 1;
pub const GROUPS_SET_REPLACE: u64 = 0;
pub const GROUPS_SET_RESERVED_NONE: u64 = 0;
pub const IDENTITY_SET_RESERVED_NONE: u64 = 0;
pub const MAX_CMDLINE_BYTES: usize = 4096;
pub const FD_CLOEXEC: u64 = 1;
pub const MAX_FDS: usize = 1024;
pub const MAX_ENV_COUNT: usize = 64;
pub const MEM_MAP_SHARED: u64 = 1 << 0;
pub const NICE_BIAS: i32 = 20;
pub const NICE_GET: u64 = 0;
pub const NICE_SET: u64 = 1;
pub const PROT_DEFAULT: u64 = PROT_READ | PROT_WRITE;
pub const PROT_EXEC: u64 = 1 << 2;
pub const PROT_READ: u64 = 1 << 0;
pub const PROT_WRITE: u64 = 1 << 1;
pub const SIGALRM: u32 = 14;
pub const SIGBUS: u32 = 7;
pub const SIGCHLD: u32 = 17;
pub const SIGCONT: u32 = 18;
pub const SIGFPE: u32 = 8;
pub const SIGILL: u32 = 4;
pub const SIGINT: u32 = 2;
pub const SIGKILL: u32 = 9;
pub const SIGPIPE: u32 = 13;
pub const SIGSEGV: u32 = 11;
pub const SIGSTOP: u32 = 19;
pub const SIGTERM: u32 = 15;
pub const SIGUSR1: u32 = 10;
pub const SIGUSR2: u32 = 12;
pub const STREAM_READ_NONBLOCK: u64 = 1;
pub const STREAM_READ_PEEK: u64 = 2;
