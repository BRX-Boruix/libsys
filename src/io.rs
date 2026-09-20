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
}

/// PRE-12 / A1-4：**两侧镜像一致性断言**（编译期钉死）。
///
/// 字面值与 kernel `vfs::inode::StatInfo` 侧的断言**逐值相同**——任一侧
/// 改字段而另一侧未同步，本侧字面断言即编译失败（S06 跨边界数据契约）。
/// 布局：node_type@0 size@8 perms@16 created@24 modified@32 changed@40
/// owner_uid@48 owner_gid@52，sizeof=56（repr(C)，尾部追加只增不改）。
const _: () = {
    assert!(core::mem::size_of::<StatInfo>() == 56, "StatInfo layout drifted: sync kernel mirror");
    assert!(core::mem::offset_of!(StatInfo, owner_uid) == 48);
    assert!(core::mem::offset_of!(StatInfo, owner_gid) == 52);
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
