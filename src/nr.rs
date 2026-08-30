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
    /// VOLUME 域（ADR-030 §决策6）：卷管理能力（挂/列/更/格/卸）。
    pub const VOLUME: u32 = 0x60;
}

pub mod op {
    pub const CREATE: u32 = 0x01;
    pub const READ: u32 = 0x02;
    pub const WRITE: u32 = 0x03;
    pub const DELETE: u32 = 0x04;
    /// VOLUME 域扩展动词：格式化卷（建文件系统）。独立于 4 个通用动词，
    /// 不复用 `DELETE` 的 0x04（业务语义不同，S13）。
    pub const FORMAT: u32 = 0x05;
    /// VOLUME 域扩展动词：卸载卷。
    pub const UNMOUNT: u32 = 0x06;
    /// DEVICE 域扩展动词（P2-2）：消费下一条硬件拓扑事件（DeviceArrived/Departed）。
    /// 供用户态 `volumed` 订阅内核块设备事件（ADR-030 §决策3 事件通道）。
    pub const EVENT: u32 = 0x07;
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
/// `stream_dup(old_fd, new_fd) -> new_fd`：复制 fd（dup2 语义，pipe-features
/// 方案 A）。把 `old_fd` 的句柄复制到 `new_fd`（先关 `new_fd` 旧句柄），副本
/// 与原句柄共享同一文件描述/管道端。管道端引用计数由内核同步维护。
pub const SYS_STREAM_DUP: u32 = nr(domain::STREAM, 0x05); // 0x15

// ---------- 2. MEMORY Domain (0x20) ----------
/// `memory_map(size, flags, shared_id) -> addr`：分配/映射虚存区。
pub const SYS_MEMORY_MAP: u32 = nr(domain::MEMORY, op::CREATE); // 0x21
/// `SYS_MEMORY_MAP` 的 `flags` 位：新建共享内存对象并映射（ADR-014 §4.2，旧
/// `SYS_SHM_CREATE` 合并路径；`shared_id` 传 0 走本路径）。`shared_id != 0`
/// 时映射既有共享对象（旧 `SYS_SHM_MAP` 合并）。
pub const MEM_MAP_SHARED: u64 = 1 << 0;
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
/// `entry_chdir(path_ptr) -> 0`：切换当前进程工作目录（VFS 域扩展）。
pub const SYS_ENTRY_CHDIR: u32 = nr(domain::VFS, 0x05); // 0x45
/// `entry_getcwd(buf_ptr, cap) -> len`：读当前进程工作目录到用户缓冲。
pub const SYS_ENTRY_GETCWD: u32 = nr(domain::VFS, 0x06); // 0x46

// ---------- 5. DEVICE Domain (0x50, UIO Sandboxing) ----------
/// `driver_register(name_ptr, len) -> uio_id`：注册用户态驱动。
pub const SYS_DRIVER_REGISTER: u32 = nr(domain::DEVICE, op::CREATE); // 0x51
/// `driver_query(dev_name_ptr, out_json_ptr, cap) -> len`：查询设备绑定状态（JSON）。
pub const SYS_DRIVER_QUERY: u32 = nr(domain::DEVICE, op::READ); // 0x52
/// `driver_claim(uio_id, mmio_base, size) -> user_vaddr`：映射设备 MMIO。
pub const SYS_DRIVER_CLAIM: u32 = nr(domain::DEVICE, op::WRITE); // 0x53
/// `driver_unregister(slot) -> 0`：注销驱动并解绑设备。
pub const SYS_DRIVER_UNREGISTER: u32 = nr(domain::DEVICE, op::DELETE); // 0x54
/// `driver_event_next(buf_ptr, cap) -> len`：消费下一条硬件拓扑事件（JSON）。
/// 无待消费事件返回 0（空）。volumed 订阅块设备事件（ADR-030 §决策3）。
pub const SYS_DRIVER_EVENT_NEXT: u32 = nr(domain::DEVICE, op::EVENT); // 0x57
/// `device_probe(name_ptr) -> status`：对指定块设备做一次缓存穿透探测读，
/// 触发其驱动真实访问设备；若设备已消失，驱动发布 `DeviceDeparted`。返回
/// `ProbeStatus`（alive=0 / gone=1 / notfound=2 / notio=3）。volumed 低频对账
/// 用它兜底发现"拔除但无事件"的空闲卷（ADR-030 热插拔闭环）。
pub const SYS_DEVICE_PROBE: u32 = nr(domain::DEVICE, 0x08); // 0x58

// ---------- 6. VOLUME Domain (0x60, ADR-030) ----------
/// `volume_mount(dev_name_ptr, out_path_ptr, out_cap) -> len`：挂载一个块设备
/// 分区到 `/volumes/{name}`，把**真实挂载路径**写入 out_path（返回其长度）。
pub const SYS_VOLUME_MOUNT: u32 = nr(domain::VOLUME, op::CREATE); // 0x61
/// `volume_list(buf_ptr, cap) -> len`：列出已挂载卷（JSON）。
pub const SYS_VOLUME_LIST: u32 = nr(domain::VOLUME, op::READ); // 0x62
/// `volume_update(path_ptr, new_path_ptr, flags) -> 0`：卷属性管理（重命名/卸载信号）。
pub const SYS_VOLUME_UPDATE: u32 = nr(domain::VOLUME, op::WRITE); // 0x63
/// `volume_format(dev_name_ptr, label_ptr) -> 0`：格式化卷（建文件系统）。
pub const SYS_VOLUME_FORMAT: u32 = nr(domain::VOLUME, op::FORMAT); // 0x65
/// `volume_unmount(path_ptr) -> 0`：卸载卷。
pub const SYS_VOLUME_UNMOUNT: u32 = nr(domain::VOLUME, op::UNMOUNT); // 0x66

/// `exec` 程序池索引：shell（PID 2）。
pub const PROG_SHELL: u64 = 1;

/// `info` 查询项常量（保留供系统信息查询使用）。
pub const INFO_VERSION: u64 = 0;
pub const INFO_BOOT_MS: u64 = 1;
pub const INFO_CPU_COUNT: u64 = 2;
