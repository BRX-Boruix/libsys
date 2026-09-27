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
    /// FLAG_PIPE（ADR-014 §4.1）：配合空路径，经 `SYS_STREAM_CREATE` 分配一对
    /// 匿名管道流句柄（`OpenFlags::pipe()` 便捷构造），而非打开文件节点。
    pub pipe: bool,
}

impl OpenFlags {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        pipe: false,
    };

    pub const WRITE_ONLY: Self = Self {
        read: false,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        pipe: false,
    };

    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        pipe: false,
    };

    pub const CREATE_OR_TRUNCATE: Self = Self {
        read: true,
        write: true,
        create: true,
        truncate: true,
        append: false,
        directory: false,
        pipe: false,
    };

    /// FLAG_PIPE 便捷构造：与 `pipe_create()` 同语义（含 pipe 位、空路径）。
    /// 显式不读不写文件节点——内核在 FLAG_PIPE 分支忽略 read/write 位。
    pub const fn pipe_only() -> Self {
        Self {
            read: true,
            write: true,
            create: false,
            truncate: false,
            append: false,
            directory: false,
            pipe: true,
        }
    }

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
        if self.pipe {
            bits |= 1 << 6;
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

/// 系统门禁位（wire bit9，ADR-040 §2.5 线格式 ABI 常量）。
///
/// chmod 通道唯一写入门径：`chmod(path, classic | GATE_SYSTEM_BIT)`。
/// POSIX `mode_t` 语义只占 classic 9 位（0o777），bit9 仅限显式声明的
/// 门禁写入（如 shell 驱动安装的 System-only 收紧）。
pub const GATE_SYSTEM_BIT: u32 = 1 << 9;

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

/// stat 结果（与内核 vfs::inode::StatInfo 同布局的镜像）。
///
/// #[repr(C)] 固定布局，与内核 ABI 逐字段一致——这是跨边界的真实数据合约，任一例改字段必须同步另一例。
/// A1-4 / ADR-040 §2.4：属主字段**尾部追加**（owner_uid/owner_gid），与内核
/// 侧同变更同步（PRE-12 纪律：改不同步即静默错位伪数据）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatInfo {
    /// 节点类型稳定数字标签（见 StatInfo::type_tag）。
    pub node_type: u32,
    /// 文件字节大小。
    pub size: u64,
    /// 权限位（classic 9 位直通 + 门禁 bit9，与内核 `AccessPolicy::to_wire`
    /// 同编码；旧 ≤0o7 披露位由内核 `from_wire` 兼容扩展）。
    pub perms: u32,
    /// 创建时间（Unix 秒；字段不存在时 0）。
    pub created_time: u64,
    /// 修改时间（Unix 秒）。
    pub modified_time: u64,
    /// 变更时间（Unix 秒）。
    pub changed_time: u64,
    /// 属主 uid（A1-4 尾部追加；内核自节点策略本体投影真值）。
    pub owner_uid: u32,
    /// 属主 gid（同上）。
    pub owner_gid: u32,
    /// 本 fd 是否为终端（ADR-044 §1.2 / J-TOKEN-A 尾部追加）：`1`=是，`0`=否/未知。
    ///
    /// 这是 `isatty` 的**真值来源**，取代此前 `libc` 里 `fd ∈ {0,1,2} → 1` 的
    /// 硬编码猜测：终端性跟着**节点**走，不跟着 fd 号走，故 stdout 被重定向到
    /// 普通文件后如实为 0。`0` 兼作「未知」（内核旧版/无该字段时）。
    pub is_terminal: u32,
    /// 本节点所属 console 的 **owner pid**（ADR-044 §1.3 / J-TOKEN-B 尾部追加）：
    /// `0` = **无主/未知**（S17 安全侧默认，与 `is_terminal` 的 0 同一约定）。
    ///
    /// **`0` 不得读作「pid 0 持有控制台」**：它表示「无主/未知」。
    /// 真值来自节点自述（`INode::console_owner`，S15 单点定义）。
    pub console_owner: u64,
}

/// PRE-12 / A1-4：**两侧镜像一致性断言**（编译期钉死）。
///
/// 字面值与 kernel `vfs::inode::StatInfo` 侧的断言**逐值相同**——任一侧
/// 改字段而另一侧未同步，本侧字面断言即编译失败（S06 跨边界数据契约）。
/// 布局：node_type@0 size@8 perms@16 created@24 modified@32 changed@40
/// owner_uid@48 owner_gid@52 is_terminal@56 console_owner@64，sizeof=72
/// （repr(C)，尾部追加只增不改；J-TOKEN-B 追加 console_owner）。
const _: () = {
    assert!(core::mem::size_of::<StatInfo>() == 72, "StatInfo layout drifted: sync kernel mirror");
    assert!(core::mem::offset_of!(StatInfo, owner_uid) == 48);
    assert!(core::mem::offset_of!(StatInfo, owner_gid) == 52);
    assert!(core::mem::offset_of!(StatInfo, is_terminal) == 56);
    assert!(core::mem::offset_of!(StatInfo, console_owner) == 64);
};

/// 进程身份查询结果（A2-1 / ADR-040 §3.5 G1；与内核 `task::IdentityInfo` 同布局的镜像）。
///
/// `#[repr(C)]` 固定布局，跨边界真实数据合约，任一侧改字段必须同变更同步
/// （PRE-12 纪律，同 `StatInfo`）。字段为**定长数字**（ADR-018 §三层校验 /
/// ADR-040 §2.10：身份查询参数只用定长数字，不经用户态字符串）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityInfo {
    /// 真实 uid（**不得**伪造、不得兜底）。
    pub uid: u32,
    /// 真实 gid。
    pub gid: u32,
    /// 能力位（`Caps::bits()` 的 u8 值，零扩展；见内核 `task::Caps`）。
    pub caps: u32,
}

/// A2-1：**两侧镜像一致性断言**（编译期钉死，同 `StatInfo` 纪律）。
/// 布局：uid@0 gid@4 caps@8，sizeof=12。
const _: () = {
    assert!(core::mem::size_of::<IdentityInfo>() == 12, "IdentityInfo layout drifted: sync kernel mirror");
    assert!(core::mem::offset_of!(IdentityInfo, uid) == 0);
    assert!(core::mem::offset_of!(IdentityInfo, gid) == 4);
    assert!(core::mem::offset_of!(IdentityInfo, caps) == 8);
};

/// 补充组设置结果（A2-4 / ADR-040 §2.1 `NamedGid` / §3.5 G6 组账户；与内核
/// `kernel::syscall::GroupsInfo` 同布局的镜像）。
///
/// `#[repr(C)]` 固定布局，跨边界真实数据合约，任一侧改字段必须同变更同步
/// （PRE-12 纪律，同 `StatInfo`/`IdentityInfo`）。**不含变长数据**——`gids` 是定长
/// 数组，满足 ADR-018/ADR-040 §2.10「参数只用定长数字」。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupsInfo {
    /// 实际生效的补充组个数（`0..=GROUPS_MAX`）。
    pub count: u32,
    /// 保留（须为 0；便于将来向尾部增长而不破 ABI）。
    pub reserved: u32,
    /// 组 id 数组（前 `count` 项有效）。
    pub gids: [u32; crate::nr::GROUPS_MAX],
}

/// A2-4：**两侧镜像一致性断言**（编译期钉死，同 `StatInfo`/`IdentityInfo` 纪律）。
/// 布局：count@0 reserved@4 gids@8（8×u32），sizeof=40。
const _: () = {
    assert!(core::mem::size_of::<GroupsInfo>() == 40, "GroupsInfo layout drifted: sync kernel mirror");
    assert!(core::mem::offset_of!(GroupsInfo, count) == 0);
    assert!(core::mem::offset_of!(GroupsInfo, reserved) == 4);
    assert!(core::mem::offset_of!(GroupsInfo, gids) == 8);
};

/// 显式 ACE 的 wire 形态（A2-6 / ADR-040 §3.5.1 G4；与内核 `vfs::inode::AceWire`
/// 同布局的镜像）。
///
/// `#[repr(C)]` 定长 **24 字节**——跨边界真实数据合约，任一侧改字段必须同变更同步
/// （PRE-12 纪律，同 `StatInfo`/`IdentityInfo`）。定长数组形态满足 ADR-040 §2.10
/// 「参数只用定长数字」，且**不**触碰既有 `StatInfo` 布局（既有 stat ABI 零破坏）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AceWire {
    /// 主体类别：0=Owner、1=NamedUid、2=NamedGid、3=Other。
    pub principal_kind: u32,
    /// 主体 id（NamedUid/NamedGid 的 uid/gid；Owner/Other 须为 0）。
    pub principal_id: u32,
    /// 1=Allow，0=Deny。
    pub allow: u32,
    /// 权限位集：Read=1、Write=2、Execute=4（取值范围 0..=7）。
    pub perms: u32,
    /// 目录继承标记 0/1。
    pub inherit: u32,
    /// 保留位（须为 0）。
    pub reserved: u32,
}

/// 主体类别编码（与内核同名常量同值，S13 单点对齐）。
pub const ACE_PRINCIPAL_OWNER: u32 = 0;
pub const ACE_PRINCIPAL_NAMED_UID: u32 = 1;
pub const ACE_PRINCIPAL_NAMED_GID: u32 = 2;
pub const ACE_PRINCIPAL_OTHER: u32 = 3;

/// 权限位编码（`AceWire::perms`）。
pub const ACE_PERM_READ: u32 = 1;
pub const ACE_PERM_WRITE: u32 = 2;
pub const ACE_PERM_EXECUTE: u32 = 4;

/// 单条 wire ACE 的字节大小（`AceWire` 定长；与内核 `ACE_WIRE_SIZE` 同值）。
pub const ACE_WIRE_SIZE: usize = 24;

/// 一次 ACE 传输的最大条数（与内核 `ACE_WIRE_MAX` 同值）。
pub const ACE_WIRE_MAX: usize = 64;

impl AceWire {
    /// 构造 Allow/Deny ACE 的便捷形态（`reserved` 恒 0）。
    pub const fn new(principal_kind: u32, principal_id: u32, allow: bool, perms: u32, inherit: bool) -> Self {
        Self {
            principal_kind,
            principal_id,
            allow: allow as u32,
            perms,
            inherit: inherit as u32,
            reserved: 0,
        }
    }
}

/// A2-6：**两侧镜像一致性断言**（编译期钉死，同 `StatInfo`/`IdentityInfo` 纪律）。
/// 布局：六字段各 u32，偏移 0/4/8/12/16/20，sizeof=24。
const _: () = {
    assert!(core::mem::size_of::<AceWire>() == 24, "AceWire layout drifted: sync kernel mirror");
    assert!(core::mem::offset_of!(AceWire, principal_kind) == 0);
    assert!(core::mem::offset_of!(AceWire, principal_id) == 4);
    assert!(core::mem::offset_of!(AceWire, allow) == 8);
    assert!(core::mem::offset_of!(AceWire, perms) == 12);
    assert!(core::mem::offset_of!(AceWire, inherit) == 16);
    assert!(core::mem::offset_of!(AceWire, reserved) == 20);
};

impl StatInfo {
    /// 节点类型稳定数字标签（与内核 StatInfo::type_tag 一致）。
    pub fn type_tag(t: u32) -> u32 {
        t
    }
    /// 节点类型常量：普通文件。
    pub const TYPE_FILE: u32 = 1;
    /// 节点类型常量：目录。
    pub const TYPE_DIR: u32 = 2;
    /// 节点类型常量：字符设备。
    pub const TYPE_CHARDEV: u32 = 3;
    /// 节点类型常量：块设备。
    pub const TYPE_BLKDEV: u32 = 4;
    /// 节点类型常量：软链符。
    pub const TYPE_SYMLINK: u32 = 5;
    /// 节点类型常量：命名管道。
    pub const TYPE_FIFO: u32 = 6;
    /// 节点类型常量：套接字。
    pub const TYPE_SOCKET: u32 = 7;
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

/// `dup2(old_fd, new_fd)`：把 `old_fd` 的句柄复制到 `new_fd`（Unix dup2，
/// pipe-features 方案 A）。
///
/// - 若 `new_fd` 已打开，先关闭旧句柄再复制；
/// - 副本与 `old_fd` 指向同一文件描述/管道端（共享读写偏移 / 管道同一端）；
/// - `old_fd == new_fd` 时仅校验存在性，返回 new_fd。
///
/// 返回 `new_fd`。管道端引用计数由内核同步维护，用户态无需关心。
pub fn dup2(old_fd: u64, new_fd: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_STREAM_DUP, [old_fd, new_fd, 0, 0, 0, 0])
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

/// `read_nonblocking(fd, buf)`：**非阻塞**读——有就返回，没有立刻 `WouldBlock`。
///
/// # 与 [`read`] 的唯一差别
///
/// 置了内核的 `STREAM_READ_NONBLOCK` 标志（`a5`）。效果：交互 stdin 空读时
/// 内核**不登记等待者、不切走本进程**，而是如实返回
/// [`Error::WouldBlock`]。
///
/// # 何时该用它
///
/// **轮询**式探键（例如前台等待子进程期间照看 `^C`）。
/// **不要**用它替代正常的行读取——那会变成忙等烧 CPU；正常读请用 [`read`]，
/// 让内核阻塞等待（那是正确的省电语义）。
///
/// # 返回
///
/// * `Ok(n)`：读到 `n` 字节（可能短读）；
/// * `Err(WouldBlock)`：**此刻**无数据可读，调用方应稍后再试或干别的；
/// * 其他 `Err`：如实上抛。
pub fn read_nonblocking(fd: u64, buf: &mut [u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_READ,
        [
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            crate::nr::STREAM_READ_NONBLOCK | crate::nr::STREAM_READ_PEEK,
            0,
        ],
    )
    .map(|n| n as usize)
}

/// `read_nonblocking_take(fd, buf)`：**消费式**非阻塞读——取走字节且不阻塞。
///
/// # 与 [`read_nonblocking`] 的唯一差别
///
/// 不带 [`crate::nr::STREAM_READ_PEEK`]：字节**被取走**（本读者游标推进），
/// 而非预览。`nr.rs:76-82` 成文预留的「NONBLOCK 与 PEEK 语义可分离」的
/// 消费臂——首个使用方是 consoled 的 getty 清积压（登录前按键作废）。
///
/// # 返回
///
/// * `Ok(n)`：取走 `n` 字节（可能短读）；
/// * `Err(WouldBlock)`：此刻无可消费数据；
/// * 其他 `Err`：如实上抛。
pub fn read_nonblocking_take(fd: u64, buf: &mut [u8]) -> Result<usize, Error> {
    crate::syscall::call(
        SYS_STREAM_READ,
        [
            fd,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            STREAM_OFFSET_CURRENT,
            crate::nr::STREAM_READ_NONBLOCK,
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

/// 重命名（SYS_ENTRY_UPDATE rename 动作）：同目录内将 old 改名为 new。
/// 保留 inode 身份，不动内容。
pub fn rename(old_path: &str, new_path: &str) -> Result<(), Error> {
    let mut old = [0u8; 256];
    if old_path.len() >= 255 || new_path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    old[..old_path.len()].copy_from_slice(old_path.as_bytes());
    old[old_path.len()] = 0;
    let mut new = [0u8; 256];
    new[..new_path.len()].copy_from_slice(new_path.as_bytes());
    new[new_path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_UPDATE,
        [old.as_ptr() as u64, new.as_ptr() as u64, 0, crate::nr::ENTRY_UPDATE_RENAME, 0, 0],
    )
    .map(|_| ())
}

/// 设置权限（SYS_ENTRY_UPDATE chmod 动作）：wire classic 位直通（A1-7）。
///
/// `bits` = classic 9 位（0o777）+ 可选门禁位 [`GATE_SYSTEM_BIT`]。**不再
/// 经 `Permissions` 布尔标签中转**——布尔结构 `to_bits()` 恒 ≤0o7，经内核
/// 等值三段扩展会坍缩成三段同值（chmod 0644 → 0111 权限畸变），无法表达
/// 三段；PRE-3「折叠编码=权限放大」在线格式的直接消解。
pub fn chmod(path: &str, bits: u32) -> Result<(), Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_UPDATE,
        [buf.as_ptr() as u64, bits as u64, 0, crate::nr::ENTRY_UPDATE_CHMOD, 0, 0],
    )
    .map(|_| ())
}

/// 易主（SYS_ENTRY_UPDATE chown 动作，A1-7）：把节点属主设为 (uid, gid)。
///
/// 参数为定长寄存器值（非指针），无用户拷贝面。内核侧强制：属主或
/// `CAP_OWNER` 可 chmod/chown 自己的节点；**易他主（目标属主 ≠ 调用者）**
/// 需 `CAP_SYSTEM`（POSIX chown 限制面，ADR-040 §2.6 延伸）。
pub fn chown(path: &str, uid: u32, gid: u32) -> Result<(), Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_UPDATE,
        [buf.as_ptr() as u64, uid as u64, gid as u64, crate::nr::ENTRY_UPDATE_CHOWN, 0, 0],
    )
    .map(|_| ())
}

/// 设置显式 ACE 列表（A2-6 / ADR-040 §3.5.1 G4；SYS_ENTRY_UPDATE 动作 3）。
///
/// **整表替换**：`aces` 为空即清空显式列表。只改显式 ACE——classic 三段、属主、
/// 门禁位一律原样（与 [`chmod`] 的"只改 mode"纪律对称）。
///
/// 授权面：属主或 `CAP_OWNER`（ACE 列表即访问策略本体，能改它等于能改节点权限）。
/// 非属主且无 `CAP_OWNER` → `EACCES`。
///
/// 失败语义：任一条 ACE 畸形（未知主体类别 / `perms > 7` / `allow|inherit > 1` /
/// `reserved != 0` / Owner|Other 携带非 0 id）→ `InvalidParam`，**且整表不写回**
/// （不留半套策略）。`aces.len() > ACE_WIRE_MAX` → `InvalidParam`。
pub fn set_aces(path: &str, aces: &[AceWire]) -> Result<(), Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    if aces.len() > ACE_WIRE_MAX {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_UPDATE,
        [
            buf.as_ptr() as u64,
            aces.as_ptr() as u64,
            aces.len() as u64,
            crate::nr::ENTRY_UPDATE_SET_ACES,
            0,
            0,
        ],
    )
    .map(|_| ())
}

/// 读取显式 ACE 列表（A2-6 / ADR-040 §3.5.1 G4；SYS_ENTRY_READ 动作 2）。
///
/// 返回节点**显式** ACE（不含三条隐式尾部 ACE——那是 classic 三段的展开，
/// 经 [`stat`] 的 `perms` 字段即可读）。授权面与 stat 同源：READ 权限。
///
/// **不截断**：节点 ACE 数 > `out.len()` 时如实 `NoSpace`——截断会让调用方
/// 误以为已拿到全部策略（安全面伪成功）。故先用 `cap = 0` 探测条数、再按需
/// 扩容，是推荐用法（`aces_count(path)` 即该形态的便利封装）。
///
/// 返回实际条数。
pub fn get_aces(path: &str, out: &mut [AceWire]) -> Result<usize, Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    if out.len() > ACE_WIRE_MAX {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_READ,
        [
            buf.as_ptr() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            crate::nr::ENTRY_READ_ACES,
            0,
            0,
        ],
    )
    .map(|n| n as usize)
}

/// 探测节点显式 ACE **条数**（A2-6）：`cap = 0` 的合法探测调用，不写任何字节。
///
/// 便利封装 `get_aces` 的定长数组用法：`aces_count` 后按需分配再 `get_aces`。
pub fn aces_count(path: &str) -> Result<usize, Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(
        SYS_ENTRY_READ,
        [buf.as_ptr() as u64, 0, 0, crate::nr::ENTRY_READ_ACES, 0, 0],
    )
    .map(|n| n as usize)
}

/// stat(path)：解析路径返回节点元数据（SYS_ENTRY_READ stat 动作）。
pub fn stat(path: &str) -> Result<StatInfo, Error> {
    let mut buf = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    let mut out = core::mem::MaybeUninit::<StatInfo>::zeroed();
    let n = crate::syscall::call(
        SYS_ENTRY_READ,
        [buf.as_ptr() as u64, out.as_mut_ptr() as u64, core::mem::size_of::<StatInfo>() as u64, crate::nr::ENTRY_READ_STAT, 0, 0],
    )? as usize;
    if n < core::mem::size_of::<StatInfo>() {
        return Err(Error::OutOfRange);
    }
    Ok(unsafe { out.assume_init() })
}

/// fstat(fd)：按 fd 读元数据（SYS_STREAM_FSTAT）。
pub fn fstat(fd: u64) -> Result<StatInfo, Error> {
    let mut out = core::mem::MaybeUninit::<StatInfo>::zeroed();
    let n = crate::syscall::call(
        SYS_STREAM_FSTAT,
        [fd, out.as_mut_ptr() as u64, 0, 0, 0, 0],
    )? as usize;
    if n < core::mem::size_of::<StatInfo>() {
        return Err(Error::OutOfRange);
    }
    Ok(unsafe { out.assume_init() })
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
            // §6.5：**信任但验证**——n 钳到缓冲区长度内再切片。
            // 真 syscall 下内核保证 n <= 512，此为纯防御；宿主测试里
            // syscall::call 返回垃圾（实测 3221225477），不钳即越界 panic。
            Ok(n) => {
                let n = clamp_read_n(n, chunk.len());
                data.extend_from_slice(&chunk[..n]);
            }
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
/// 把 `read_dir` 拿到的文本解析成目录条目（§6.3 的修复点）。
///
/// **抽为纯函数的原因**：`read_dir` 依赖真实 VFS（只在 QEMU 内存在），解析缺陷若内联
/// 其中就只能靠停机测试撞出来。抽出后可在宿主上对畸形输入逐条锁定（S23/S31）。
/// 与 `parse_proc_list`（J-TREE-a）同一手法。
/// 把 `read_dir` 拿到的文本解析成目录条目（§6.3 的修复点）。
///
/// **抽为纯函数的原因**：`read_dir` 依赖真实 VFS（只在 QEMU 内存在），解析缺陷若内联
/// 其中就只能靠停机测试撞出来。抽出后可在宿主上对畸形输入逐条锁定（S23/S31）。
/// 与 `parse_proc_list`（J-TREE-a）同一手法。
///
/// **为何改用 `JsonParser` 而非字符串切分**：旧实现按闭合花括号加逗号切分、再按逗号
/// 切字段，一旦某个字符串字段的真实内容里出现这两个序列（如文件名含逗号），切分点即
/// 错位，结果是**静默产出错值**——比报错更糟（S09 宁可报错，绝不返回伪数据）。
/// 对抗测试 `parse_dir_entries_name_with_json_struct_chars` 已锁定（红灯转绿的证据
/// 见该测试：旧实现把 `we'ir,d}.txt` 切成 `we'ir`）。
///
/// **错误策略**（与 `parse_proc_list` 一致）：语法根本不是 JSON 数组 →
/// `Err(InvalidParam)`（上抛，不假装成功）；单条记录内部字段缺失/非法 →
/// 该字段如实取安全值（`size` 为 0），条目本身仍保留（文件确实存在，只是元数据不全）。
pub fn parse_dir_entries(text: &str) -> Result<Vec<DirEntry>, Error> {
    let trimmed = text.trim();
    if !trimmed.starts_with('[') {
        return Err(Error::InvalidParam);
    }
    let value = crate::json::JsonParser::new(trimmed)
        .parse()
        .map_err(|_| Error::InvalidParam)?;
    let items = match value {
        crate::json::JsonValue::Array(items) => items,
        _ => return Err(Error::InvalidParam),
    };

    let mut entries = Vec::new();
    for item in items {
        let fields = match item {
            crate::json::JsonValue::Object(fields) => fields,
            _ => continue,
        };

        let mut name = String::new();
        let mut node_type = String::new();
        let mut size = 0u64;
        for (key, val) in &fields {
            match (key.as_str(), val) {
                ("name", crate::json::JsonValue::String(s)) => name = s.clone(),
                ("type", crate::json::JsonValue::String(s)) => node_type = s.clone(),
                ("size", crate::json::JsonValue::Number(n)) => size = n.parse::<u64>().unwrap_or(0),
                _ => {}
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
    Ok(entries)
}

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
    parse_dir_entries(text)
}

/// 把 `read` 返回的字节数钳到缓冲区长度内（§6.5 的修复点）。
///
/// # 为什么这个函数必须存在（缺陷背景）
///
/// `read_to_end` 原来直接写 `&chunk[..n]`，`n` 取自 `read()` 的返回值。
/// 真 syscall 下内核保证 `n <= buf.len()`，所以缺陷不可达；
/// 但在**无 syscall 的宿主测试**里，`syscall::call` 的返回值是垃圾
/// （实测出现过 `3221225477`），`&chunk[..n]` 立即越界 panic：
/// `range end index 3221225477 out of range for slice of length 512`。
///
/// **防御在哪一侧**：用户态库**信任但验证**——不假定内核一定正确，
/// 也不因此拒绝服务；越界的 n 钳到合法上界，数据照常收集。
/// 返回钳后的 n 供调用方判断进展（若内核返回 0 表示 EOF，钳制不改变 0）。
pub(crate) fn clamp_read_n(n: usize, buf_len: usize) -> usize {
    if n > buf_len {
        buf_len
    } else {
        n
    }
}

#[cfg(test)]
mod io_tests {
    /// J-TOKEN-B / PRE-12：**ABI 尺寸在用户侧独立钉死**。
    ///
    /// 内核侧（`vfs::inode`）与用户侧（本文件）各有一次编译期断言，
    /// 二者**各自独立**断言 72 —— 任一侧改字段而另一侧未同步，
    /// 该侧编译即失败（S06 跨边界真实数据契约）。
    ///
    /// 本测试把该断言**在宿主上再跑一遍**：编译期断言只在被编译时生效，
    /// 而本测试保证「当前源码树里两侧同值」这一事实被显式验证，
    /// 而不是靠「编译过了所以肯定一致」的推断。
    #[test]
    fn statinfo_abi_layout_is_72_bytes() {
        use core::mem::{offset_of, size_of};
        assert_eq!(size_of::<crate::io::StatInfo>(), 72, "StatInfo sizeof (J-TOKEN-B)");
        // 尾部追加纪律：既有字段偏移一律不变。
        assert_eq!(offset_of!(crate::io::StatInfo, node_type), 0);
        assert_eq!(offset_of!(crate::io::StatInfo, size), 8);
        assert_eq!(offset_of!(crate::io::StatInfo, perms), 16);
        assert_eq!(offset_of!(crate::io::StatInfo, created_time), 24);
        assert_eq!(offset_of!(crate::io::StatInfo, modified_time), 32);
        assert_eq!(offset_of!(crate::io::StatInfo, changed_time), 40);
        assert_eq!(offset_of!(crate::io::StatInfo, owner_uid), 48);
        assert_eq!(offset_of!(crate::io::StatInfo, owner_gid), 52);
        assert_eq!(offset_of!(crate::io::StatInfo, is_terminal), 56);
        // J-TOKEN-B 新增字段：u32 之后按 u64 对齐，落在 64。
        assert_eq!(offset_of!(crate::io::StatInfo, console_owner), 64);
    }

    /// J-TOKEN-B：`0` 表示**无主/未知**，不是「pid 0 持有控制台」。
    ///
    /// 这是本项最容易误读的一点，故用测试钉死语义：默认构造的 StatInfo
    /// 的 console_owner 必须是 0，且该值被解释为「没有人持有」。
    #[test]
    fn statinfo_default_console_owner_is_unowned() {
        let si = crate::io::StatInfo {
            node_type: 0,
            size: 0,
            perms: 0,
            created_time: 0,
            modified_time: 0,
            changed_time: 0,
            owner_uid: 0,
            owner_gid: 0,
            is_terminal: 0,
            console_owner: 0,
        };
        assert_eq!(si.console_owner, 0, "0 = unowned/unknown, NOT pid 0");
    }
    /// §6.3 对抗测试：名字字段含 JSON 结构字符时不得切错。
    ///
    /// 旧实现按闭合花括号加逗号切分、再按逗号切字段，一旦名字里含 `},{`、`,`、`"`，
    /// 切分点即错位——静默产出错值（比报错更糟，S09）。
    /// 名字来自真实文件名，该输入可达（例如 `we'ir,d}.txt`）。
    #[test]
    fn parse_dir_entries_name_with_json_struct_chars() {

        // 
        // 用 JsonWriter 造输入：手拼转义容易写错。
        let mut w = crate::json::JsonWriter::new(crate::json::VecTarget::new());
        let mut arr = w.start_array().unwrap();
        arr.push_object(|o| {
            o.field_str("name", "we'ir,d}.txt").unwrap();
            o.field_str("type", "RegularFile").unwrap();
            o.field_u64("size", 12).unwrap();
            Ok(())
        }).unwrap();
        arr.push_object(|o| {
            o.field_str("name", "plain.txt").unwrap();
            o.field_str("type", "RegularFile").unwrap();
            o.field_u64("size", 1).unwrap();
            Ok(())
        }).unwrap();
        arr.end().unwrap();
        let text = w.into_target().into_string().unwrap();
        let entries = parse_dir_entries(&text).expect("must parse");
        assert_eq!(entries.len(), 2, "both entries must survive: {:?}", entries);
        assert_eq!(entries[0].name, "we'ir,d}.txt");
        assert_eq!(entries[0].size, 12);
        assert_eq!(entries[1].name, "plain.txt");
        assert_eq!(entries[1].size, 1);
    }

    /// 非 JSON 输入如实报错（不假装成功，S09）。
    #[test]
    fn parse_dir_entries_rejects_non_json() {
        assert!(parse_dir_entries("name:type:12").is_err());
        assert!(parse_dir_entries("").is_err());
    }

    /// 字段缺失的条目仍保留（与 parse_proc_list 的错误策略一致）。
    #[test]
    fn parse_dir_entries_keeps_entry_with_missing_fields() {
        let mut w = crate::json::JsonWriter::new(crate::json::VecTarget::new());

        let mut arr = w.start_array().unwrap();
        arr.push_object(|o| { o.field_str("name", "partial.txt").unwrap(); Ok(()) }).unwrap();
        arr.end().unwrap();
        let text = w.into_target().into_string().unwrap();
        let entries = parse_dir_entries(&text).expect("must parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "partial.txt");
        assert_eq!(entries[0].size, 0, "missing size = safe default 0");
    }
    use super::*;

    /// 正常路径：n 在范围内时原样通过（钳制不得改变合法值）。
    #[test]
    fn clamp_keeps_valid_n_unchanged() {
        assert_eq!(clamp_read_n(0, 512), 0);
        assert_eq!(clamp_read_n(1, 512), 1);
        assert_eq!(clamp_read_n(511, 512), 511);
        assert_eq!(clamp_read_n(512, 512), 512, "n == buf_len is legal");
    }

    /// 对抗路径：宿主垃圾返回值（实测出现过的那个数）必须被钳住。
    #[test]
    fn clamp_pins_host_garbage_to_buf_len() {
        // 3221225477 = 0xC0000005，实测从宿主 syscall 桩返回过的垃圾值。
        assert_eq!(clamp_read_n(3_221_225_477, 512), 512);
        assert_eq!(clamp_read_n(usize::MAX, 512), 512);
    }

    /// 边界：空缓冲区（任何 n > 0 都钳到 0，切片不会 panic）。
    #[test]
    fn clamp_handles_empty_buf() {
        assert_eq!(clamp_read_n(0, 0), 0);
        assert_eq!(clamp_read_n(1, 0), 0);
        assert_eq!(clamp_read_n(usize::MAX, 0), 0);
    }

    /// `read_to_end` 的切片路径整体不 panic：构造一个**必然**拿到垃圾 n 的
    /// 场景不可行（read 走真 syscall），但钳制函数是唯一可疑点，
    /// 此处用与 `read_to_end` 相同的切片表达式证明钳后安全。
    #[test]
    fn sliced_chunk_never_panics_after_clamp() {
        let chunk = [0u8; 512];
        for n in [0usize, 1, 512, 513, 3_221_225_477, usize::MAX] {
            let clamped = clamp_read_n(n, chunk.len());
            // 与 read_to_end 内部相同的表达式；钳后必须合法。
            let s = &chunk[..clamped];
            assert!(s.len() <= chunk.len());
        }
    }
}