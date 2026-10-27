//! auxv（辅助向量）访问——ABI v2（3P4-2）起入口字数组在 envp 之后追加 auxv 对。
//!
//! # 定位规则（docs/abi/syscall-abi.md §4 的消费端单点）
//!
//! auxv 紧随 envp 的 NULL 终结槽之后，形如 (类型, 值) 成对排布，以 (AT_NULL, 0) 终结。
//! 本模块只做**读取**：当前内核交付的槽是 AT_EXECFN（可执行文件名）。
//!
//! # 为什么程序名要单独一个槽
//!
//! BORUIX 的 argv[0] 是**整条命令行**（不是程序名，见 §4.1），该语义已发布、不得改变。
//! "进程从哪里被 exec 起来"因此需要独立来源——AT_EXECFN（与 Linux 同值 31）。

/// auxv 槽类型：可执行文件名（与 Linux 的 AT_EXECFN 同值 31）。
pub const AT_EXECFN: u64 = crate::abi_generated::AT_EXECFN;

/// auxv 终结槽类型（值恒 0）。
pub const AT_NULL: u64 = crate::abi_generated::AT_NULL;

/// auxv 对数上限的**镜像常量**：内核侧单点在 loader 的同一常量族。
///
/// 用户态与内核分属两个仓，无法共享字面量，故以文档 + 两侧测试互锚。用作**防御性
/// 扫描边界**：超过它仍未见到 AT_NULL 即判定契约被破坏，返回 None（不静默截断）。
pub const MAX_AUXV_PAIRS: usize = 16;

/// 由 argc/argv 定位 auxv（不含 AT_NULL 终结项）；契约被破坏时返回 None。
///
/// # Safety
/// argc/argv 必须来自入口（user_main 的两个实参）。
#[inline]
pub unsafe fn auxv<'a>(argc: isize, argv: *const *const u8) -> Option<&'a [(u64, u64)]> {
    let env = crate::env::envp(argc, argv)?;
    // auxv 紧随 envp 的 NULL 终结槽之后。
    let base = env.as_ptr().add(env.len() + 1) as *const u64;
    let mut n = 0usize;
    loop {
        if *base.add(n * 2) == AT_NULL {
            return Some(core::slice::from_raw_parts(
                base as *const (u64, u64),
                n,
            ));
        }
        n += 1;
        if n > MAX_AUXV_PAIRS {
            return None;
        }
    }
}

/// 动态链接所需的五个槽（阶段 5 / 3P5-1，取值与 Linux 一致）。
///
/// 它们由内核**只在本镜像要求解释器时**写入（静态程序的 auxv 与本模块历史形态逐字一致，
/// 零影响）。解释器据此知道：给谁重定位（`AT_PHDR`/`AT_PHNUM`）、跳哪去（`AT_ENTRY`）、
/// 它自己装在哪（`AT_BASE` = 0 表示 ET_EXEC 解释器，没有基址可报）。
pub const AT_PHDR: u64 = 3;
pub const AT_PHENT: u64 = 4;
pub const AT_PHNUM: u64 = 5;
pub const AT_BASE: u64 = 7;
pub const AT_ENTRY: u64 = 9;

/// 解释器启动时从 auxv 读到的那一份**转交契约**。
///
/// 五者缺一不可：少 `AT_PHDR` 就找不到程序头（无法知道要重定位什么），少 `AT_ENTRY`
/// 就不知道该跳到哪。故本结构**不做部分填充**——任一项缺席即整体 `None`，
/// 由调用方如实退回"非解释器模式"（S09：宁可不做，不做半个）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterpAux {
    /// 可执行文件的程序头表在用户地址空间里的 VA。
    pub phdr: u64,
    /// 单个程序头条目的字节数。
    pub phent: u64,
    /// 程序头条数。
    pub phnum: u64,
    /// 可执行文件入口（解释器做完重定位后跳这里）。
    pub entry: u64,
    /// 解释器自己的装载基址；0 = ET_EXEC 解释器（无基址可报）。
    pub base: u64,
}

/// 读取转交契约（只在"本进程是被内核当解释器装载起来的"时有值）。
///
/// # Safety
/// 同 auxv。
#[inline]
pub unsafe fn interp_aux(argc: isize, argv: *const *const u8) -> Option<InterpAux> {
    let table = auxv(argc, argv)?;
    let mut phdr = None;
    let mut phent = None;
    let mut phnum = None;
    let mut entry = None;
    let mut base = None;
    for (t, v) in table {
        match *t {
            AT_PHDR => phdr = Some(*v),
            AT_PHENT => phent = Some(*v),
            AT_PHNUM => phnum = Some(*v),
            AT_ENTRY => entry = Some(*v),
            AT_BASE => base = Some(*v),
            _ => {}
        }
    }
    // 五项齐备才算数：缺任何一项都说明内核没按解释器路径装载本进程（或契约被破坏）。
    Some(InterpAux {
        phdr: phdr?,
        phent: phent?,
        phnum: phnum?,
        entry: entry?,
        base: base?,
    })
}
/// 取 AT_EXECFN：本进程的**可执行文件名**（**不是** argv[0]——后者是整条命令行）。
///
/// # Safety
/// 同 auxv。
#[inline]
pub unsafe fn execfn<'a>(argc: isize, argv: *const *const u8) -> Option<&'a [u8]> {
    let table = auxv(argc, argv)?;
    for (t, v) in table {
        if *t != AT_EXECFN {
            continue;
        }
        if *v == 0 {
            return None;
        }
        let p = *v as *const u8;
        let mut len = 0usize;
        while len <= crate::cmdline::MAX_CMDLINE_BYTES {
            if *p.add(len) == 0 {
                return Some(core::slice::from_raw_parts(p, len));
            }
            len += 1;
        }
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// 构造符合 ABI v2 的入口字数组：
    /// [argc] ++ argv[0..argc] ++ [NULL] ++ envp... ++ [NULL] ++ auxv 对 ++ [AT_NULL, 0]。
    /// 返回值指向**argv[0] 槽**（argc = 0 时即 NULL 终结槽）。
    fn build(argc: isize, argv0: u64, env: &[&[u8]], auxv: &[(u64, u64)]) -> Vec<*const u8> {
        let mut words: Vec<*const u8> = Vec::new();
        words.push(argc as usize as *const u8);
        if argc == 1 {
            words.push(argv0 as usize as *const u8);
        }
        words.push(core::ptr::null());
        for e in env {
            words.push(e.as_ptr());
        }
        words.push(core::ptr::null());
        for (t, v) in auxv {
            words.push(*t as usize as *const u8);
            words.push(*v as usize as *const u8);
        }
        words.push(AT_NULL as usize as *const u8);
        words.push(core::ptr::null());
        words
    }

    #[test]
    fn execfn_returns_program_name() {
        let name: &[u8] = b"tlsdemo\0";
        let words = build(1, 0x1000, &[], &[(AT_EXECFN, name.as_ptr() as u64)]);
        let argv = unsafe { words.as_ptr().add(1) };
        assert_eq!(unsafe { execfn(1, argv) }, Some(&b"tlsdemo"[..]));
        // auxv 表里应恰有一项（AT_NULL 不计入）。
        assert_eq!(unsafe { auxv(1, argv) }.map(|t| t.len()), Some(1));
    }

    #[test]
    fn execfn_none_when_absent() {
        // 只有 AT_NULL：无 AT_EXECFN 时返回 None（不猜、不回落成 argv[0]）。
        let words = build(1, 0x1000, &[], &[]);
        let argv = unsafe { words.as_ptr().add(1) };
        assert_eq!(unsafe { execfn(1, argv) }, None);
        assert_eq!(unsafe { auxv(1, argv) }.map(|t| t.len()), Some(0));
    }

    /// 五项齐备才给出转交契约；缺任何一项即 `None`（不做部分填充）。
    #[test]
    fn interp_aux_requires_all_five_entries() {
        let full = [
            (AT_PHDR, 0x40_0040),
            (AT_PHENT, 56),
            (AT_PHNUM, 6),
            (AT_ENTRY, 0x40_1250),
            (AT_BASE, 0x0020_0000_0000),
        ];
        let words = build(1, 0x1000, &[], &full);
        let argv = unsafe { words.as_ptr().add(1) };
        let got = unsafe { interp_aux(1, argv) };
        assert_eq!(
            got,
            Some(InterpAux {
                phdr: 0x40_0040,
                phent: 56,
                phnum: 6,
                entry: 0x40_1250,
                base: 0x0020_0000_0000,
            })
        );
        // 少任意一项都必须整体缺席——半个契约比没有更危险（解释器会拿 0 当地址用）。
        for drop in 0..full.len() {
            let mut partial: Vec<(u64, u64)> = full.to_vec();
            partial.remove(drop);
            let w = build(1, 0x1000, &[], &partial);
            let a = unsafe { w.as_ptr().add(1) };
            assert_eq!(unsafe { interp_aux(1, a) }, None, "缺第 {} 项时不得给出契约", drop);
        }
    }

    /// 静态程序（只有 AT_EXECFN）：没有转交契约，`None`——解释器据此走普通模式。
    #[test]
    fn interp_aux_absent_for_static_programs() {
        let name: &[u8] = b"prog\0";
        let words = build(1, 0x1000, &[], &[(AT_EXECFN, name.as_ptr() as u64)]);
        let argv = unsafe { words.as_ptr().add(1) };
        assert_eq!(unsafe { interp_aux(1, argv) }, None);
    }
    #[test]
    fn auxv_returns_none_when_unterminated() {
        let mut words: Vec<*const u8> = Vec::new();
        words.push(1usize as *const u8);
        words.push(0x1000usize as *const u8);
        words.push(core::ptr::null()); // argv NULL
        words.push(core::ptr::null()); // envp NULL
        for _ in 0..(MAX_AUXV_PAIRS + 2) {
            words.push(AT_EXECFN as usize as *const u8);
            words.push(0x2000usize as *const u8);
        }
        let argv = unsafe { words.as_ptr().add(1) };
        assert!(unsafe { auxv(1, argv) }.is_none(), "未终结必须返回 None");
    }
}
