//! 系统调用号定义——遵循 ADR-014 4x4 对象-动词正交架构与规范。
//!
//! 编码统一采用 `(Resource << 4) | Verb`：
//! - Resources: STREAM (0x10), MEMORY (0x20), TASK (0x30), VFS (0x40), DEVICE (0x50)
//! - Verbs:     CREATE (0x01), READ (0x02), WRITE (0x03), DELETE (0x04)
//!
//! 用户态通过 `int 0x80` 触发，寄存器约定：`rax=nr`, `rdi/rsi/rdx/r10/r8/r9`。

// ---------- 4 类资源域与 4 个动词 ----------

pub mod domain {
    pub const STREAM: u32 = 0x10;
    pub const MEMORY: u32 = 0x20;
    pub const TASK: u32 = 0x30;
    pub const VFS: u32 = 0x40;
    pub const DEVICE: u32 = 0x50;
}

pub mod op {
    pub const CREATE: u32 = 0x01;
    pub const READ: u32 = 0x02;
    pub const WRITE: u32 = 0x03;
    pub const DELETE: u32 = 0x04;
}

const fn nr(d: u32, o: u32) -> u32 {
    d | o
}

/// STREAM read/write 的顺序 I/O 哨兵值。
///
/// 仅该值表示使用并推进句柄当前位置；`0` 与所有其他偏移均表示定位
/// `pread`/`pwrite`，其中 `0` 是文件起始位置。
pub const STREAM_OFFSET_CURRENT: u64 = u64::MAX;

// ---------- 1. STREAM Domain (0x10) ----------
/// `stream_create(path_ptr, flags, mode) -> handle`：打开或创建流/文件。
pub const SYS_STREAM_CREATE: u32 = nr(domain::STREAM, op::CREATE); // 0x11
/// `stream_read(handle, buf_ptr, len, offset) -> n`：仅 `offset=STREAM_OFFSET_CURRENT` 为顺序读；其余（含 0）为定位读。
pub const SYS_STREAM_READ: u32 = nr(domain::STREAM, op::READ); // 0x12
/// `stream_write(handle, buf_ptr, len, offset) -> n`：仅 `offset=STREAM_OFFSET_CURRENT` 为顺序写；其余（含 0）为定位写。
pub const SYS_STREAM_WRITE: u32 = nr(domain::STREAM, op::WRITE); // 0x13
/// `stream_close(handle) -> 0`：关闭并释放流句柄。
pub const SYS_STREAM_CLOSE: u32 = nr(domain::STREAM, op::DELETE); // 0x14

// ---------- 2. MEMORY Domain (0x20) ----------
/// `memory_map(size, flags, shared_id) -> addr`：分配/映射虚存区。
pub const SYS_MEMORY_MAP: u32 = nr(domain::MEMORY, op::CREATE); // 0x21
/// `memory_query(addr, out_ptr) -> 0`：查询 `addr` 所在 4KB 页属性，把 u64
/// 位图写入 `out_ptr`（须为可写用户缓冲）。KM2：内核侧此前缺席本调用号，
/// 现已接通；位值与内核 `sys_memory_query` 双侧定义、注释互指。
pub const SYS_MEMORY_QUERY: u32 = nr(domain::MEMORY, op::READ); // 0x22
/// `memory_grow(new_break) -> break`：调整进程堆边界（替代 brk）。
pub const SYS_MEMORY_GROW: u32 = nr(domain::MEMORY, op::WRITE); // 0x23
/// `memory_unmap(addr, size) -> 0`：解除虚存映射。
pub const SYS_MEMORY_UNMAP: u32 = nr(domain::MEMORY, op::DELETE); // 0x24

/// `memory_query` 位图：页表项 present（demand 区未触碰时整字为 0）。
pub const MEMQ_PRESENT: u64 = 1 << 0;
/// `memory_query` 位图：用户态可访问（PTE user 位）。
pub const MEMQ_USER: u64 = 1 << 1;
/// `memory_query` 位图：可写（PTE rw 位）。
pub const MEMQ_WRITABLE: u64 = 1 << 2;

// ---------- 3. TASK Domain (0x30) ----------
/// `task_spawn(path_ptr, args_ptr, args_len) -> pid`：加载 ELF 镜像为新进程执行。
pub const SYS_TASK_SPAWN: u32 = nr(domain::TASK, op::CREATE); // 0x31
/// `task_wait(target_pid, timeout_ns) -> status`：等待任务退出或睡眠/让出。
pub const SYS_TASK_WAIT: u32 = nr(domain::TASK, op::READ); // 0x32
/// `task_signal(target_pid, signal) -> 0`：向任务发送控制/终止信号。
pub const SYS_TASK_SIGNAL: u32 = nr(domain::TASK, op::WRITE); // 0x33
/// `task_exit(code) -> !`：终止当前任务。
pub const SYS_TASK_EXIT: u32 = nr(domain::TASK, op::DELETE); // 0x34

// ---------- 4. VFS Domain (0x40) ----------
/// `entry_create(path_ptr, kind, perm) -> 0`：创建目录或特殊节点。
pub const SYS_ENTRY_CREATE: u32 = nr(domain::VFS, op::CREATE); // 0x41
/// ENTRY_CREATE 的 kind：创建目录（ADR-014 §4.4 `kind=DIR/DIRECTORY`）。
pub const ENTRY_KIND_DIRECTORY: u64 = 0;
/// ENTRY_CREATE 的 kind：创建普通文件。
pub const ENTRY_KIND_FILE: u64 = 1;
/// `entry_read(path_ptr, json_buf_ptr, cap) -> len`：读取目录项列表（直接填充 JSON）。
pub const SYS_ENTRY_READ: u32 = nr(domain::VFS, op::READ); // 0x42
/// `entry_update(path_ptr, new_path_ptr, flags) -> 0`：移动/重命名/修改元数据。
pub const SYS_ENTRY_UPDATE: u32 = nr(domain::VFS, op::WRITE); // 0x43
/// `entry_delete(path_ptr) -> 0`：删除节点（替代 unlink）。
pub const SYS_ENTRY_DELETE: u32 = nr(domain::VFS, op::DELETE); // 0x44

// ---------- 5. DEVICE Domain (0x50, UIO Sandboxing) ----------
/// `driver_register(name_ptr, len) -> uio_id`：注册用户态驱动。
pub const SYS_DRIVER_REGISTER: u32 = nr(domain::DEVICE, op::CREATE); // 0x51
/// `driver_claim(uio_id, mmio_base, size) -> user_vaddr`：映射设备 MMIO。
pub const SYS_DRIVER_CLAIM: u32 = nr(domain::DEVICE, op::WRITE); // 0x53

/// `exec` 程序池索引：shell（PID 2）。
pub const PROG_SHELL: u64 = 1;

/// `info` 查询项常量（保留供系统信息查询使用）。
pub const INFO_VERSION: u64 = 0;
pub const INFO_BOOT_MS: u64 = 1;
pub const INFO_CPU_COUNT: u64 = 2;
