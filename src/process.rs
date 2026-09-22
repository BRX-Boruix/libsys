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
