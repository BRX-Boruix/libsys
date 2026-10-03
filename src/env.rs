//! 进程环境（envp）访问——ABI v2（3P4-2）起入口栈参数块在 argv 之后携带 envp。
//!
//! # 定位规则（docs/abi/syscall-abi.md §4 的消费端单点）
//!
//! envp 首址 = argv + (argc + 1) * 8（argv 即 argv[0] 槽地址）。该式对 argc ∈ {0, 1}
//! 一致成立，故**无需新 ABI 槽**。envp 以 NULL 指针终结，每项指向字符串区内一条
//! "K=V" 串（NUL 结尾）。
//!
//! # 与 cmdline 的分工
//!
//! cmdline 处理"内核不拆词"的命令行（不是 POSIX argv）；本模块处理 POSIX 形态的环境
//! 数组。两者都从同一入口参数块读取，边界扫描一律**有界**（防御性上界，S02）。

/// envp 条数上限的**镜像常量**：内核侧单点在 loader 的 MAX_ENV_COUNT。
///
/// 用户态与内核分属两个仓，无法共享字面量，故以文档 + 两侧测试互锚。用作**防御性扫描
/// 边界**：超过它仍未见到 NULL 终结符即判定契约被破坏，返回 None（不静默截断）。
pub const MAX_ENV_COUNT: usize = 64;

/// 由 argc/argv 取 envp 切片（不含 NULL 终结槽）；契约被破坏时返回 None。
///
/// # Safety
/// argc/argv 必须来自入口（user_main 的两个实参）——本函数按该契约解引用。
#[inline]
pub unsafe fn envp<'a>(argc: isize, argv: *const *const u8) -> Option<&'a [*const u8]> {
    if argc < 0 || argv.is_null() {
        return None;
    }
    // 跳过 argc 个 argv 项与其 NULL 终结槽。
    let base = argv.add(argc as usize + 1);
    let mut n = 0usize;
    loop {
        if (*base.add(n)).is_null() {
            return Some(core::slice::from_raw_parts(base, n));
        }
        n += 1;
        if n > MAX_ENV_COUNT {
            // 超上界仍无终结符：契约被破坏（宁缺毋假，S09）。
            return None;
        }
    }
}

/// 环境变量迭代器（产出 "K=V" 原始字节串，不假设 UTF-8）。
pub struct Env<'a> {
    items: &'a [*const u8],
    idx: usize,
}

impl<'a> Iterator for Env<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let p = *self.items.get(self.idx)?;
        self.idx += 1;
        if p.is_null() {
            return None;
        }
        // 逐字节扫描到 NUL；同样有界（防御性上界）。
        let mut len = 0usize;
        while len <= crate::cmdline::MAX_CMDLINE_BYTES {
            if unsafe { *p.add(len) } == 0 {
                return Some(unsafe { core::slice::from_raw_parts(p, len) });
            }
            len += 1;
        }
        None
    }
}

/// 遍历当前进程环境（argc/argv 来自入口）。
///
/// # Safety
/// 同 envp。
#[inline]
pub unsafe fn env<'a>(argc: isize, argv: *const *const u8) -> Option<Env<'a>> {
    Some(Env {
        items: envp(argc, argv)?,
        idx: 0,
    })
}

/// 取环境变量 name 的值（name **不含** '='）。
///
/// # Safety
/// 同 envp。
#[inline]
pub unsafe fn var<'a>(argc: isize, argv: *const *const u8, name: &[u8]) -> Option<&'a [u8]> {
    let mut it = env(argc, argv)?;
    while let Some(item) = it.next() {
        if item.len() > name.len() && item[..name.len()] == *name && item[name.len()] == b'=' {
            return Some(&item[name.len() + 1..]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// 构造符合 ABI v2 的入口字数组：[argc] ++ argv[0..argc] ++ [NULL] ++ envp... ++ [NULL]。
    /// 调用方取的 argv 指针指向 **argv[0] 槽**；argc = 0 时该槽即 NULL 终结槽。
    fn build(argc: isize, argv0: u64, env: &[&[u8]]) -> Vec<*const u8> {
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
        words
    }

    #[test]
    fn envp_locates_after_argv_null() {
        // 环境串按 ABI 必须 NUL 结尾（真实交付即如此）。
        let e1: &[u8] = b"PATH=/programs\0";
        let e2: &[u8] = b"TERM=boruix\0";
        let words = build(1, 0x1000, &[e1, e2]);
        // argv 指向 **argv[0] 槽**（即 rsp + 8），不是 argc 槽——这是 ABI §4 的消费端约定。
        let argv = unsafe { words.as_ptr().add(1) };
        let got = unsafe { envp(1, argv) }.expect("envp must be locatable");
        assert_eq!(got.len(), 2, "envp 条数 = 环境条数");
        let mut it = unsafe { env(1, argv) }.expect("env");
        assert_eq!(it.next(), Some(&b"PATH=/programs"[..]));
        assert_eq!(it.next(), Some(&b"TERM=boruix"[..]));
        assert_eq!(it.next(), None, "NULL 终结后迭代结束");
    }

    #[test]
    fn envp_works_for_empty_cmdline() {
        // argc = 0 时 argv[0] 槽即 NULL 终结槽，envp 仍在其后（定位规则对两种 argc 一致）。
        let e1: &[u8] = b"HOME=/\0";
        let words = build(0, 0, &[e1]);
        let argv = unsafe { words.as_ptr().add(1) };
        let mut it = unsafe { env(0, argv) }.expect("env");
        assert_eq!(it.next(), Some(&b"HOME=/"[..]));
    }

    #[test]
    fn var_returns_value_and_rejects_prefix_only() {
        let e1: &[u8] = b"PATH=/programs\0";
        let e2: &[u8] = b"PATHEXTRA=no\0";
        let words = build(1, 0, &[e1, e2]);
        let argv = unsafe { words.as_ptr().add(1) };
        assert_eq!(
            unsafe { var(1, argv, b"PATH") },
            Some(&b"/programs"[..]),
            "命中完整键名"
        );
        // 前缀相同但键名不同（PATH vs PATHEXTRA）不得误命中。
        assert_eq!(unsafe { var(1, argv, b"PATHX") }, None);
        assert_eq!(unsafe { var(1, argv, b"NOPE") }, None);
    }

    #[test]
    fn envp_returns_none_when_unterminated() {
        // 人为去掉 NULL 终结槽：必须返回 None（有界扫描，不越界读）。
        let e: &[u8] = b"A=B\0";
        let mut words: Vec<*const u8> = vec![
            1usize as *const u8,
            0x1000usize as *const u8,
            core::ptr::null(),
        ];
        for _ in 0..(MAX_ENV_COUNT + 2) {
            words.push(e.as_ptr());
        }
        let argv = unsafe { words.as_ptr().add(1) };
        assert!(unsafe { envp(1, argv) }.is_none());
    }
}
