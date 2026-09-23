//! 任务与进程（TASK 域）薄封装（遵循 ADR-014）。

use crate::error::Error;
use crate::nr::{
    DERIVE_FLAGS_NONE, GROUPS_SET_CLEAR, GROUPS_SET_REPLACE, GROUPS_SET_RESERVED_NONE,
    IDENTITY_SET_RESERVED_NONE, SYS_TASK_DERIVE, SYS_TASK_EXIT, SYS_TASK_GETPID, SYS_TASK_GETTID,
    SYS_TASK_GROUPS_SET, SYS_TASK_IDENTITY_QUERY, SYS_TASK_IDENTITY_SET, SYS_TASK_SIGNAL,
    SYS_TASK_SPAWN, SYS_TASK_WAIT,
};

/// `exec(prog, cmd)`：加载程序（可为内建索引或路径）为新进程（PID 2 等）并运行，返回新进程 pid。
pub fn exec(prog: u64, cmd: &[u8]) -> Result<u64, Error> {
    crate::syscall::call(
        SYS_TASK_SPAWN,
        [prog, cmd.as_ptr() as u64, cmd.len() as u64, 0, 0, 0],
    )
}

/// `exec_path(path, cmd)`：直接从 VFS 路径（如 `/programs/shell.elf`）加载并运行新进程。
pub fn exec_path(path: &str, cmd: &[u8]) -> Result<u64, Error> {
    let mut null_terminated = [0u8; 256];
    if path.len() >= 255 {
        return Err(Error::OutOfRange);
    }
    null_terminated[..path.len()].copy_from_slice(path.as_bytes());
    null_terminated[path.len()] = 0;

    crate::syscall::call(
        SYS_TASK_SPAWN,
        [
            null_terminated.as_ptr() as u64,
            cmd.as_ptr() as u64,
            cmd.len() as u64,
            0,
            0,
            0,
        ],
    )
}

/// 进程快照条目。
///
/// **ABI 注意**：`#[repr(C, align(8))]` 是对外布局契约——字段顺序与填充已由
/// 既有调用方（`shell` / `ps`）依赖，**新增字段只能追加在末尾**，不得插队。
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct PsEntry {
    /// 进程 id。
    pub pid: u32,
    /// 状态：0=未知 1=Ready 2=Running 3=Blocked。
    ///
    /// **0 是未知而非 Ready**：`/processes/list` 未来若出现未登记的状态名，
    /// 默认成 Ready 就是伪数据（S09），故显式保留 0 表示内核报了看不懂的状态。
    pub state: u8,
    /// 填充（保留）。
    pub _pad: [u8; 3],
    /// 父进程 id（0 = 无父进程，如 `init`）。
    ///
    /// 真值来源：`vfs/src/procfs.rs` 的 `ppid` 字段（内核 `Process::ppid`）。
    /// **作业树（ADR-043 支柱 1）据此在用户态构造父子关系**——内核零改动。
    pub ppid: u32,
}

impl PsEntry {
    /// 空条目（缓冲初始化用）。字段全 0 即无此进程——`pid == 0` 是非法进程号，
    /// 解析时被显式丢弃，故 0 不会被误当成真实进程。
    pub const EMPTY: PsEntry = PsEntry {
        pid: 0,
        state: 0,
        _pad: [0; 3],
        ppid: 0,
    };
}

/// 把 `/processes/list` 的 JSON 文本解析进 `buf`，返回写入条目数。
///
/// **抽为纯函数的原因**：`ps()` 依赖真实 VFS（只在 QEMU 内存在），解析缺陷若内联
/// 其中就只能靠停机测试撞出来。抽出后可在宿主上对畸形输入逐条锁定（S23/S31）。
///
/// **为何改用 `JsonParser` 而非字符串切分**：旧实现按闭合花括号加逗号切分、再按逗号
/// 切字段，一旦某个字符串字段的真实内容里出现这两个序列（如进程名含逗号），切分点即
/// 错位，结果是**静默产出错值**——比报错更糟（S09 宁可报错，绝不返回伪数据）。
///
/// **错误策略**：语法根本不是 JSON 数组 → `Err(InvalidParam)`（上抛，不假装成功）；
/// 单条记录内部字段缺失/非法 → 该字段如实取安全值（`ppid`/`state` 为 0），
/// 条目本身仍保留（进程确实存在，只是元数据不全）。
pub fn parse_proc_list(text: &str, buf: &mut [PsEntry]) -> Result<usize, Error> {
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

    let mut count = 0;
    for item in items {
        if count >= buf.len() {
            break;
        }
        let fields = match item {
            crate::json::JsonValue::Object(fields) => fields,
            _ => continue,
        };

        let mut pid = 0u32;
        let mut ppid = 0u32;
        let mut state = 0u8;
        for (key, val) in &fields {
            match key.as_str() {
                "pid" => pid = json_u32(val).unwrap_or(0),
                "ppid" => ppid = json_u32(val).unwrap_or(0),
                "state" => {
                    state = match val {
                        crate::json::JsonValue::String(s) => match s.as_str() {
                            "Ready" => 1,
                            "Running" => 2,
                            "Blocked" => 3,
                            _ => 0,
                        },
                        _ => 0,
                    }
                }
                _ => {}
            }
        }

        if pid == 0 {
            continue;
        }

        buf[count] = PsEntry {
            pid,
            state,
            _pad: [0; 3],
            ppid,
        };
        count += 1;
    }
    Ok(count)
}


// ===========================================================================
// J-TREE-b（ADR-043 支柱 1）：作业树构造（纯逻辑）
// ===========================================================================

/// 求全部**根**进程（无父，或父已不在表中），按 pid 升序。
///
/// **关键语义（S09）**：父已退出的**孤儿不得凭空消失**。若只认 `ppid == 0`,
/// 则父退出后其子进程会从作业树上静默掉落——用户看到的是「进程没了」，
/// 而它其实还活着。故父 pid 不在表中时，该进程按根呈现。
///
/// 纯逻辑：不读 VFS、不分配堆（除返回的 `Vec`），可在宿主上逐条锁定边界（S23/S31）。
pub fn job_roots(procs: &[PsEntry]) -> alloc::vec::Vec<u32> {
    fn present(procs: &[PsEntry], pid: u32) -> bool {
        procs.iter().any(|p| p.pid == pid)
    }
    let mut roots: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    for p in procs {
        // 无父，或父不存在（已退出）→ 根。自环（ppid == pid）也是根，
        // 否则它会既不是根、又永远挂在自己下面。
        let is_root = p.ppid == 0 || p.ppid == p.pid || !present(procs, p.ppid);
        if is_root {
            roots.push(p.pid);
        }
    }
    roots.sort_unstable();
    roots
}

/// 求以 `root` 为根的**作业子树** = 该进程及其全部后代，按 pid 升序。
///
/// ADR-043 决策 1：「作业 = 某个直接子进程及其全部后代」（进程树的子树）。
/// 这就是作业级操作（对整个作业发信号）的数据源。
///
/// **必须终止**：表可能被对手损坏成环路（A→B→A）。用「已访问集合」保证
/// 每个 pid 最多进入结果一次，故环路不会导致无界展开或死循环。
pub fn job_subtree(procs: &[PsEntry], root: u32) -> alloc::vec::Vec<u32> {
    let mut out: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    if !procs.iter().any(|p| p.pid == root) {
        return out;
    }
    out.push(root);
    // 广度优先逐层吸收直接子进程；`out` 兼作已访问集合。
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        i += 1;
        for p in procs {
            if p.ppid == parent && !out.contains(&p.pid) {
                out.push(p.pid);
            }
        }
    }
    out.sort_unstable();
    out
}


/// 求进程 `pid` 在进程树中的**深度**（根为 0）。
///
/// `ps` 用它在人类可读输出里缩进呈现父子归属。
///
/// **必须有界**：表可能被对手损坏成环（A→B→A）或自环（ppid == pid）。
/// 沿链回溯最多走 `procs.len() + 1` 步，超限即停——绝不无限循环、绝不 panic。
/// 父不存在（已退出）或 `ppid == 0` → 深度 0（当作根）。
pub fn job_depth(procs: &[PsEntry], pid: u32) -> u32 {
    let start = match procs.iter().find(|p| p.pid == pid) {
        Some(p) => p,
        None => return 0,
    };
    let mut ppid = start.ppid;
    let mut depth = 0u32;
    let limit = procs.len() as u32 + 1;
    while ppid != 0 && depth < limit {
        match procs.iter().find(|p| p.pid == ppid) {
            Some(parent) => {
                if parent.ppid == parent.pid {
                    break; // 自环
                }
                ppid = parent.ppid;
                depth += 1;
            }
            None => break, // 父已退出 → 当作根
        }
    }
    depth
}


/// 作业树的一行（供 `jobs` 渲染，也供宿主测试断言）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JobLine {
    /// 作业号（1 基）。0 = 该行是作业成员的续行，不占作业号。
    pub job: usize,
    /// 该行对应的 pid。
    pub pid: u32,
    /// 是否为作业根（false = 该作业的子进程）。
    pub is_root: bool,
}

/// 把一个作业（根 pid + 全部后代）展平成**渲染行序列**。
///
/// ADR-043 决策 1：作业 = 某直接子进程及其全部后代。`jobs` 的展示口径是
/// 「根一行，其余后代各一行且缩进」，本函数即该口径的**单点定义**——
/// 渲染与断言共用它，避免「测试测的和实际打印的是两套逻辑」（S06）。
///
/// 根不在 `alive` 中（已退出）时仍返回根一行（诚实呈现「Done」），
/// 后代则按真实表列出（可能为空）。
pub fn job_lines(alive: &[PsEntry], root: u32, job_no: usize) -> alloc::vec::Vec<JobLine> {
    let mut out: alloc::vec::Vec<JobLine> = alloc::vec::Vec::new();
    out.push(JobLine { job: job_no, pid: root, is_root: true });
    for pid in job_subtree(alive, root) {
        if pid != root {
            out.push(JobLine { job: 0, pid, is_root: false });
        }
    }
    out
}

/// 从 JSON 值取 `u32`；数值以字符串词法保存（ADR-013），故走字符串解析。
fn json_u32(v: &crate::json::JsonValue) -> Option<u32> {
    match v {
        crate::json::JsonValue::Number(s) => s.parse::<u32>().ok(),
        _ => None,
    }
}

/// `ps(buf) -> count`：从 `/processes/list` VFS 虚拟文件读取并解析存活进程快照。
pub fn ps(buf: &mut [PsEntry]) -> Result<usize, Error> {
    let data = crate::io::read_to_end("/processes/list")?;
    let text = core::str::from_utf8(&data).map_err(|_| Error::InvalidParam)?;
    parse_proc_list(text, buf)
}

/// 动态获取当前所有存活进程的快照列表（自动扩容）。
pub fn ps_list() -> Result<alloc::vec::Vec<PsEntry>, Error> {
    let mut entries = alloc::vec![PsEntry::EMPTY; 32];
    let count = ps(&mut entries)?;
    entries.truncate(count);
    Ok(entries)
}

/// `kill(pid, sig) -> 0`：向进程发送信号（统一走 SYS_TASK_SIGNAL）。
pub fn kill(pid: u64, sig: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_SIGNAL, [pid, sig, 0, 0, 0, 0])
}

/// `gettid() -> tid`：返回调用线程自己的 pid（线程 id，threads.md T2-6）。BORUIX 每线程一个
/// pid；组长 pid==tgid，组员 pid==线程 id。POSIX 线程据此查自身线程 id（写进其 Tcb.tid）。
pub fn gettid() -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_GETTID, [0, 0, 0, 0, 0, 0])
}

/// `getpid() -> pid`：返回所在线程组组长 pid（POSIX 进程 id / tgid）。替代读 /processes/list 扫
/// Running 的脆弱启发（多线程/SMP 下会挑错成员）。
pub fn getpid() -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_GETPID, [0, 0, 0, 0, 0, 0])
}

/// `derive(flags, entry_rsp, entry_rip) -> pid`：**COW 派生子进程**（ADR-038）。
///
/// 以调用进程为父派生**新线程组**的子进程：用户地址空间与父共享全部已映射数据
/// 帧（写时复制），fd/cwd/identity 按 ADR-038 决策逐项继承。子进程在父被本调用
/// 中断处继续执行。
///
/// **返回语义（POSIX fork 铁律）**：父收新子进程 pid（> 0）、子收 0。
///
/// `flags` / `entry_rsp` / `entry_rip` 首期必须全为 [`DERIVE_FLAGS_NONE`]（= 0，
/// 表示继承父当前 RIP/RSP）；非 0 时内核如实返回 `InvalidParam`。
///
/// **注意**：本函数返回两次（父一次、子一次），这是 POSIX `fork()` 语义的本质，
/// 不是错误。调用方**必须**按返回值分流；libc 层的 `fork()` 即据此包装。
pub fn derive(flags: u64, entry_rsp: u64, entry_rip: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_TASK_DERIVE, [flags, entry_rsp, entry_rip, 0, 0, 0])
}

/// `derive_inherit() -> pid`：[`derive`] 的**首期唯一合法形态**——继承父当前 RIP/RSP。
///
/// 三个保留参数显式钉为 [`DERIVE_FLAGS_NONE`]（=0），使「继承」这一语义在调用点
/// 成文，而非依赖调用方记得传 0（S13：不留魔法值）。将来 ABI 扩展（带入口的派生）
/// 会新增独立的命名构造函数，本函数语义**不变**。
pub fn derive_inherit() -> Result<u64, Error> {
    derive(DERIVE_FLAGS_NONE, DERIVE_FLAGS_NONE, DERIVE_FLAGS_NONE)
}

/// `exit(code)`：终止当前进程。永不返回。
pub fn exit(code: i32) -> ! {
    let _ = crate::syscall::invoke(SYS_TASK_EXIT, code as u64, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}

/// `yield_now()`：当前进程主动让出 CPU（走 SYS_TASK_WAIT(0, 0)）。
pub fn yield_now() -> Result<(), Error> {
    crate::syscall::call(SYS_TASK_WAIT, [0, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `TASK_WAIT` 的 `target_pid` 哨兵值：等待任意子进程退出。
/// 与内核侧 `task::scheduler::WAIT_ANY` 同值（`usize::MAX` / `u64::MAX`）。
pub const WAIT_ANY: u64 = u64::MAX;

/// waitpid 收割结果：被收尸子进程的 pid 与其退出码（POSIX waitpid 返回 pid、
/// status 承载退出码的语义在 libc 层拆分，此处两者一并真实交付）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitResult {
    /// 被收尸的子进程 pid。
    pub pid: u64,
    /// 子进程退出码。
    pub code: u64,
}

/// `waitpid_any()`：等待任意直接子进程退出，返回被收尸子进程的 `(pid, code)`。
/// 阻塞当前进程直到任一子进程退出。无子进程时返回 `Err(NotFound)`。
/// 内核交付协议：rax=退出码、r10=pid（同步路径经 aux_pid、阻塞路径经
/// saved.r10，两条路径一致），故用 `invoke_capture_r10` 同时取回两者。
pub fn waitpid_any() -> Result<WaitResult, Error> {
    let (ret, r10) = crate::syscall::invoke_capture_r10(SYS_TASK_WAIT, WAIT_ANY, 0, 0, 0, 0, 0);
    if ret & (1u64 << 63) != 0 {
        return Err(crate::error::Error::from_errno((ret as i64).wrapping_neg() as i32));
    }
    Ok(WaitResult { pid: r10, code: ret })
}

/// `waitpid_any_timeout(timeout_ns)`：**有界**等待任意直接子进程退出（§6.11 裁决 B）。
///
/// 最多等待 `timeout_ns` 纳秒：
/// * 子进程在期限内退出 → `Ok(WaitResult { pid, code })`（与 [`waitpid_any`] 同语义）；
/// * **期限内没等到** → `Err(WouldBlock)`——**如实**告知「还没等到、可重试」，
///   **绝不**编造一个退出码（S09：宁可如实报未等到，也不给会误导的假成功）；
/// * 无子进程/目标非法 → `Err(NotFound)`。
///
/// ## 为什么需要它
///
/// `waitpid_any()` 在子进程运行期间会**真阻塞**，只在子进程退出时被唤醒。
/// 若调用方还想在等待期间**做别的事**（如轮询 stdin 以便前台子进程运行时
/// 也能响应 `^C`），就必须用有界形态：等到就处理退出，超时就先去干别的事、
/// 再回来等。
///
/// ## 边界纪律（调用方须知）
///
/// 超时**不会**收走子进程，也**不会**改变子进程状态——它只是「本次没等到」。
/// 调用方须自行保存「我在等谁」并重复调用（本函数内部即 waitpid_any 语义，
/// 故可安全重试）。
pub fn waitpid_any_timeout(timeout_ns: u64) -> Result<WaitResult, Error> {
    let (ret, r10) = crate::syscall::invoke_capture_r10(
        SYS_TASK_WAIT,
        WAIT_ANY,
        timeout_ns,
        0,
        0,
        0,
        0,
    );
    if ret & (1u64 << 63) != 0 {
        return Err(crate::error::Error::from_errno((ret as i64).wrapping_neg() as i32));
    }
    Ok(WaitResult { pid: r10, code: ret })
}
/// 进程身份查询结果（A2-1；与内核 `kernel::syscall::IdentityInfo` 同布局镜像）。
///
/// `#[repr(C)]` 固定布局，跨边界真实数据合约（PRE-12 纪律，同 `io::StatInfo`）；
/// 字段全为定长数字（ADR-018/ADR-040 §2.10）。真正的定义在 `io` 模块，此处
/// 重导出以免调用方跨模块找结构（S13 单点定义、单点引用）。
pub use crate::io::IdentityInfo;

/// `identity_query() -> IdentityInfo`：查询**本进程**的真实 uid/gid/caps（A2-1 /
/// ADR-040 §3.5 G1）。
///
/// 只读、无门禁；不提供查询任意 pid 的形态。内核在无当前进程（内核/驱动
/// 上下文）时如实 `PermissionDenied`——不会返回 0/0/0 的伪身份（S09）。
pub fn identity_query() -> Result<IdentityInfo, Error> {
    let mut info = IdentityInfo { uid: 0, gid: 0, caps: 0 };
    crate::syscall::call(
        SYS_TASK_IDENTITY_QUERY,
        [&mut info as *mut IdentityInfo as u64, 0, 0, 0, 0, 0],
    )?;
    Ok(info)
}

/// `identity_set(uid, gid, caps) -> ()`：变更**本进程组**的身份（A2-1 / ADR-040
/// §3.5 G1，路线 B 完整 setuid 语义）。
///
/// 授权按 `CAP_SYSTEM` 二分（内核单点判定，S13）：
/// - 无 `CAP_SYSTEM`：只能降权或不变——uid 不得改变、caps 不得新增位，否则
///   内核如实返回 `PermissionDenied`；
/// - 持 `CAP_SYSTEM`：可设为任意 uid/gid（login 认证后据此降权至目标用户）。
///
/// 身份是**进程组级**的：本调用影响调用线程所在组的全部成员。
pub fn identity_set(uid: u32, gid: u32, caps: u32) -> Result<(), Error> {
    crate::syscall::call(
        SYS_TASK_IDENTITY_SET,
        [
            uid as u64,
            gid as u64,
            IDENTITY_SET_RESERVED_NONE,
            caps as u64,
            0,
            0,
        ],
    )?;
    Ok(())
}

/// 补充组设置结果（A2-4；与内核 `kernel::syscall::GroupsInfo` 同布局镜像）。
///
/// `#[repr(C)]` 固定布局，跨边界真实数据合约（PRE-12 纪律）；真正的定义在
/// `io` 模块，此处重导出以免调用方跨模块找结构（S13 单点定义、单点引用）。
pub use crate::io::GroupsInfo;

/// `groups_set(gids) -> GroupsInfo`：**整体替换**本进程组的补充组集合（A2-4 /
/// ADR-040 §2.1 `NamedGid`）。
///
/// 组表 `/config/groups.json` 由**用户态**解析后经本调用装入身份——内核不解析
/// 组表（ADR-040 §2.9 分层原则；Q6 不变）。
///
/// 授权按 `CAP_SYSTEM` 二分（内核单点判定，S13）：
/// - 无 `CAP_SYSTEM`：只能**收缩或不变**——集合必须是当前集合的子集，否则
///   内核如实返回 `PermissionDenied`（新增自己不属于的组即组越权）；
/// - 持 `CAP_SYSTEM`：可设为任意集合（login 按组表装配成员身份用，A2-7）。
///
/// **超限如实报错**：`gids.len() > GROUPS_MAX` 时内核返回 `OutOfRange`，绝不
/// 静默截断——截断会让调用方以为自己加入了某个组而实际没有（能力谎言，S09）。
/// 返回值为**实际生效**的集合，调用方无需再次查询即可确知结果。
pub fn groups_set(gids: &[u32]) -> Result<GroupsInfo, Error> {
    let mut info = GroupsInfo { count: 0, reserved: 0, gids: [0; crate::nr::GROUPS_MAX] };
    crate::syscall::call(
        SYS_TASK_GROUPS_SET,
        [
            gids.as_ptr() as u64,
            gids.len() as u64,
            GROUPS_SET_RESERVED_NONE,
            GROUPS_SET_REPLACE,
            &mut info as *mut GroupsInfo as u64,
            0,
        ],
    )?;
    Ok(info)
}

/// `groups_clear()`：清空本进程组的补充组集合（A2-4）。语义同 `groups_set(&[])`，
/// 但经专用方向码走内核的 `GROUPS_SET_CLEAR` 分支（不必传空数组指针）。
pub fn groups_clear() -> Result<GroupsInfo, Error> {
    let mut info = GroupsInfo { count: 0, reserved: 0, gids: [0; crate::nr::GROUPS_MAX] };
    crate::syscall::call(
        SYS_TASK_GROUPS_SET,
        [0, 0, GROUPS_SET_RESERVED_NONE, GROUPS_SET_CLEAR, &mut info as *mut GroupsInfo as u64, 0],
    )?;
    Ok(info)
}

// ===========================================================================
// 宿主单测（`cargo test -p libsys`）：覆盖 `/processes/list` 的 JSON 解析。
//
// **为何把解析抽成纯函数**：`ps()` 依赖真实 VFS 读（只在 QEMU 里存在），若解析
// 逻辑内联其中，任何解析缺陷都只能靠内核停机测试或人工交互撞出来——那是温室
// 测试（S29/S30）。抽成 `parse_proc_list` 后，畸形 JSON、缺字段、非法数值等
// 对抗输入可在宿主上逐条锁定（S31）。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 造一条与 `vfs/src/procfs.rs` 真实输出**同形**的 JSON 记录。
    fn entry(pid: u32, name: &str, state: &str, ppid: u32) -> alloc::string::String {
        alloc::format!(
            "{{\"pid\":{pid},\"name\":\"{name}\",\"state\":\"{state}\",\"ppid\":{ppid},\"memory_bytes\":4096,\"uri\":\"/processes/{pid}/status\"}}"
        )
    }

    fn wrap(entries: &[alloc::string::String]) -> alloc::string::String {
        let mut s = alloc::string::String::from("[");
        for (i, e) in entries.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(e);
        }
        s.push(']');
        s.push('\n');
        s
    }

    #[test]
    fn test_parse_proc_list_reads_ppid() {
        let text = wrap(&[
            entry(1, "init", "Ready", 0),
            entry(2, "shell", "Running", 1),
            entry(7, "selftest", "Blocked", 2),
        ]);
        let mut buf = [PsEntry::EMPTY; 8];
        let n = parse_proc_list(&text, &mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(buf[0].pid, 1);
        assert_eq!(buf[0].ppid, 0, "init 无父进程，ppid 必须为 0");
        assert_eq!(buf[1].pid, 2);
        assert_eq!(buf[1].ppid, 1, "shell 的父亲是 init");
        assert_eq!(buf[2].pid, 7);
        assert_eq!(buf[2].ppid, 2, "selftest 的父亲是 shell");
        assert_eq!(buf[2].state, 3, "Blocked");
    }

    #[test]
    fn test_parse_proc_list_missing_ppid_is_zero() {
        // 缺 ppid 字段：如实置 0（"无父进程"），绝不用 pid 或其它值顶替（S09）。
        let text = r#"[{"pid":5,"name":"x","state":"Ready"}]"#;
        let mut buf = [PsEntry::EMPTY; 4];
        let n = parse_proc_list(text, &mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0].ppid, 0);
    }

    #[test]
    fn test_parse_proc_list_non_numeric_ppid_is_zero() {
        // 非法数值：置 0，绝不 panic、绝不部分解析出脏值。
        let text = r#"[{"pid":5,"state":"Ready","ppid":"abc"}]"#;
        let mut buf = [PsEntry::EMPTY; 4];
        let n = parse_proc_list(text, &mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0].ppid, 0);
    }

    #[test]
    fn test_parse_proc_list_stops_at_buffer_capacity() {
        // 资源耗尽：条目数超过缓冲容量时必须**截断而非越界写**（S18/S31）。
        let entries: alloc::vec::Vec<_> = (1..=10).map(|i| entry(i, "p", "Ready", i - 1)).collect();
        let text = wrap(&entries);
        let mut buf = [PsEntry::EMPTY; 3];
        let n = parse_proc_list(&text, &mut buf).unwrap();
        assert_eq!(n, 3, "容量 3 只能返回 3 条，不得越界");
        assert_eq!(buf[0].pid, 1);
        assert_eq!(buf[2].pid, 3);
    }

    #[test]
    fn test_parse_proc_list_adversarial() {
        // 对抗输入（S31），分两类：**格式非法**必须报错（绝不假装成功），
        // **格式合法但内容异常**必须给出安全值（绝不产出伪数据）。

        // ---- 类一：格式非法 → 必须 Err，绝不返回伪造的"0 条" ----
        let malformed: &[&str] = &[
            "",                                 // 空输入
            "{}",                               // 非数组（对象）
            "boom",                             // 纯垃圾
            r#"[{"pid":1,"state":"Ready"}"#,  // 未闭合数组
            "[{",                               // 截断对象
            "[1,2",                             // 截断数字
        ];
        for input in malformed {
            let mut buf = [PsEntry::EMPTY; 4];
            let r = parse_proc_list(input, &mut buf);
            assert!(r.is_err(), "畸形输入 {input:?} 必须报错，实得 {r:?}");
        }

        // ---- 类二：格式合法但内容异常 → 安全值，不得 panic ----
        // 空数组：合法，0 条。
        assert_eq!(parse_proc_list("[]", &mut [PsEntry::EMPTY; 4]).unwrap(), 0);
        // 尾随字节（procfs 会追加 '\n'）：值本身合法 → 正常解析，不报错。
        assert_eq!(
            parse_proc_list("[1,2,3]\n", &mut [PsEntry::EMPTY; 4]).unwrap(),
            0
        );
        // pid=0 是非法进程号（同时是空槽标记），必须被丢弃。
        assert_eq!(
            parse_proc_list(r#"[{"pid":0,"state":"Ready","ppid":0}]"#, &mut [PsEntry::EMPTY; 4])
                .unwrap(),
            0
        );
        // 元素是字符串而非对象：跳过该条，不 panic、不计入。
        assert_eq!(
            parse_proc_list(r#"[{"pid":1,"ppid":0,"state":"Ready"},"junk"]"#, &mut [PsEntry::EMPTY; 4])
                .unwrap(),
            1
        );
        // 超大 pid（超出 u32）：安全值 0 → 该条被丢弃，绝不截断成别的进程号。
        assert_eq!(
            parse_proc_list(r#"[{"pid":4294967296,"state":"Ready","ppid":0}]"#, &mut [PsEntry::EMPTY; 4])
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_parse_proc_list_name_containing_separators() {
        // 回归锁定：进程名里含 "," 与 "}," 时，旧的手工切分实现会错位产出脏值。
        // 这是"静默出错值"的典型场景（S09），必须由 JSON 解析器正确读出。
        let text = r#"[{"pid":3,"name":"a,b} c","state":"Running","ppid":2}]"#;
        let mut buf = [PsEntry::EMPTY; 4];
        let n = parse_proc_list(text, &mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0].pid, 3);
        assert_eq!(buf[0].ppid, 2);
        assert_eq!(buf[0].state, 2);
    }

    #[test]
    fn test_parse_proc_list_unknown_state_is_zero() {
        // 未知状态：置 0（未知），绝不错认成 Ready（若默认 1 即为伪数据）。
        let text = r#"[{"pid":9,"state":"Zombie","ppid":1}]"#;
        let mut buf = [PsEntry::EMPTY; 4];
        let n = parse_proc_list(text, &mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0].state, 0, "未知状态必须置 0，不得默认 Ready");
        assert_eq!(buf[0].ppid, 1);
    }
}

// ===========================================================================
// J-TREE-b（ADR-043 支柱 1）：作业树构造
//
// 契约先行（S23）：先写断言，再看实现是否满足。树形构造是纯逻辑——输入
// `PsEntry` 切片，输出父子关系。抽为纯函数使其可在宿主上对边界逐条锁定，
// 而不是只能靠在 QEMU 里肉眼看 `jobs` 输出。
// ===========================================================================
#[cfg(test)]
mod tree_tests {
    use super::*;

    fn e(pid: u32, ppid: u32) -> PsEntry {
        PsEntry { pid, state: 1, _pad: [0; 3], ppid }
    }

    /// 单进程无父：自身即根。
    #[test]
    fn test_tree_single_root() {
        let procs = [e(1, 0)];
        let roots = job_roots(&procs);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], 1);
    }

    /// 真实父子链：`init(1) -> shell(2) -> child(3)`，只有 1 是根。
    #[test]
    fn test_tree_chain_has_one_root() {
        let procs = [e(1, 0), e(2, 1), e(3, 2)];
        let roots = job_roots(&procs);
        assert_eq!(roots, alloc::vec![1], "only init is a root");
    }

    /// 多个根（多个内核直创进程）按 pid 升序，顺序稳定可复现。
    #[test]
    fn test_tree_multiple_roots_sorted() {
        let procs = [e(7, 0), e(3, 0), e(5, 3)];
        let roots = job_roots(&procs);
        assert_eq!(roots, alloc::vec![3, 7]);
    }

    /// **关键边界：孤儿**。父已退出（ppid 指向不存在的 pid）时，
    /// 该进程**不能凭空消失**——否则 `jobs`/作业树会静默漏进程（S09）。
    /// 它在用户态被当作根呈现。
    #[test]
    fn test_tree_orphan_becomes_root() {
        let procs = [e(1, 0), e(9, 42)];
        let roots = job_roots(&procs);
        assert_eq!(roots, alloc::vec![1, 9], "orphan must not vanish");
    }

    /// 取某进程的全部后代（作业 = 子树），含自身。
    #[test]
    fn test_subtree_of_job_root() {
        // 1 -> 2 -> {3, 4}, 4 -> 5; 另有无关的 8 -> 9
        let procs = [e(1, 0), e(2, 1), e(3, 2), e(4, 2), e(5, 4), e(8, 0), e(9, 8)];
        let sub = job_subtree(&procs, 2);
        assert_eq!(sub, alloc::vec![2, 3, 4, 5], "job = root + all descendants");
    }

    /// 子树不越界到别人的分支。
    #[test]
    fn test_subtree_excludes_other_branch() {
        let procs = [e(1, 0), e(2, 1), e(3, 2), e(8, 0), e(9, 8)];
        let sub = job_subtree(&procs, 2);
        assert_eq!(sub, alloc::vec![2, 3], "must not include 8/9");
    }

    /// 环路必须终止（对手：损坏/伪造的表）。绝不死循环。
    #[test]
    fn test_cycle_terminates() {
        let procs = [e(1, 2), e(2, 1)];
        let roots = job_roots(&procs);
        assert!(roots.is_empty(), "a pure cycle has no root, but must not hang");
        let sub = job_subtree(&procs, 1);
        assert!(sub.contains(&1) && sub.contains(&2), "cycle members reachable once");
        assert_eq!(sub.len(), 2, "each pid appears exactly once");
    }

    /// 自环（ppid == pid）不得把自己当自己的后代反复展开。
    #[test]
    fn test_self_loop() {
        let procs = [e(5, 5)];
        let sub = job_subtree(&procs, 5);
        assert_eq!(sub, alloc::vec![5]);
    }

    /// 空表：没有任何根，也不 panic。
    #[test]
    fn test_empty_table() {
        let procs: [PsEntry; 0] = [];
        assert!(job_roots(&procs).is_empty());
        assert!(job_subtree(&procs, 1).is_empty());
    }
    /// `job_depth`：父不存在 → 0（当作根）。
    #[test]
    fn test_depth_orphan_is_root() {
        let p = [e(9, 42)];
        assert_eq!(job_depth(&p, 9), 0);
    }

    /// `job_depth`：真实链 1 -> 2 -> 3。
    #[test]
    fn test_depth_chain() {
        let p = [e(1, 0), e(2, 1), e(3, 2)];
        assert_eq!(job_depth(&p, 1), 0);
        assert_eq!(job_depth(&p, 2), 1);
        assert_eq!(job_depth(&p, 3), 2);
    }

    /// `job_depth`：成环必须终止。
    #[test]
    fn test_depth_cycle_terminates() {
        let p = [e(1, 2), e(2, 1)];
        let d = job_depth(&p, 2);
        assert!(d <= p.len() as u32 + 1, "must be bounded by table size");
    }

    /// `job_depth`：自环不吃自己。
    #[test]
    fn test_depth_self_loop() {
        let p = [e(5, 5)];
        assert_eq!(job_depth(&p, 5), 0);
    }

    /// `job_depth`：pid 不在表中 → 0。
    #[test]
    fn test_depth_unknown_pid() {
        let p = [e(1, 0)];
        assert_eq!(job_depth(&p, 77), 0);
    }
    /// `job_lines`：作业根一行 + 后代各一行，顺序与 `job_subtree` 一致。
    #[test]
    fn test_job_lines_root_and_children() {
        // 1 -> 2 -> {3, 4}
        let p = [e(1, 0), e(2, 1), e(3, 2), e(4, 2)];
        let lines = job_lines(&p, 2, 1);
        assert_eq!(lines.len(), 3, "root + two descendants");
        assert_eq!(lines[0], JobLine { job: 1, pid: 2, is_root: true });
        assert_eq!(lines[1], JobLine { job: 0, pid: 3, is_root: false });
        assert_eq!(lines[2], JobLine { job: 0, pid: 4, is_root: false });
    }

    /// `job_lines`：根已退出（不在 alive 中）仍呈现根一行。
    #[test]
    fn test_job_lines_dead_root_still_listed() {
        let p = [e(1, 0)];
        let lines = job_lines(&p, 99, 3);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], JobLine { job: 3, pid: 99, is_root: true });
    }

    /// `job_lines`：无子进程的作业只有一行。
    #[test]
    fn test_job_lines_leaf_job() {
        let p = [e(1, 0), e(5, 1)];
        let lines = job_lines(&p, 5, 2);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].pid, 5);
    }

    /// §6.12.5（所有者裁决甲）：`STREAM_READ_NONBLOCK` 取值必须与内核一致。
    ///
    /// # 为什么这条测试必须存在（PRE-12 同步守卫）
    ///
    /// 该标志是**内核与用户态的共享 ABI**：用户态传 `a5`，内核按位判定。
    /// 两侧各有一份常量定义（`libsys/src/nr.rs` 与 `kernel/.../syscall.rs`），
    /// 它们**无法互相 import**（不同 crate、不同 target）。
    ///
    /// 一旦有人只改一侧，故障是**静默**的：用户态传 1，内核若认为它是 2，
    /// 判定就不成立 —— 探键悄悄退回**阻塞**语义，`^C` 挂死缺陷原样复活，
    /// 而编译器不会有任何抱怨。这正是「静默语义漂移」，必须用测试钉死。
    ///
    /// 本测试断言的是**字面值**，与内核侧 `test_read_nonblock_flag` 里的
    /// 同名断言**成对**：改任一侧都会让另一侧的测试变红，
    /// 从而强制改动者同时更新内核常量、用户态常量与两处文档。
    #[test]
    fn test_stream_read_nonblock_matches_kernel_abi() {
        // 1：内核 `kernel/src/syscall.rs` 的 `STREAM_READ_NONBLOCK`。
        // 改这个数就必须同时改内核侧——否则两侧对同一比特的理解不一致。
        assert_eq!(crate::nr::STREAM_READ_NONBLOCK, 1);
        // 该常量处于 `a5` 的**最低位**：这一点也有意义——
        // 低位便于将来在同一个 `a5` 里再叠别的标志（如 O_NDELAY 语义扩展），
        // 而不必挪动既有位。若改成非最低位，本断言会强制复核这一决定。
        assert_eq!(crate::nr::STREAM_READ_NONBLOCK & 1, 1, "应为最低位");
    }
}
