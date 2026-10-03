//! 命令行参数访问（入口栈参数块 mini-ABI，docs/abi/syscall-abi.md §4）。
//!
//! **这不是 POSIX argv**：内核把整条命令行作为**一条字符串**放在 `argv[0]`，
//! `argc` 恒为 1，且**不拆词**——拆词是用户程序的职责。经 shell 派生时该串
//! 只含参数（shell 已剥首词）。
//!
//! 本模块把这份契约的用户态一侧收成**单点定义**（S15）：程序不再各写一份
//! 解析。原始字节语义（不假设 UTF-8，S02）——返回 `&[u8]`，编码判断留给调用方。

/// 命令行字符串区的容量上限（字节，不含 NUL 终止符）。
///
/// wire 镜像：内核侧为 loader 的 `MAX_CMDLINE_BYTES`（**单点定义**，3P4-2 起为 4096；
/// 此前内核缓冲 4096 与 loader 字符串区 0x1FF 是两个门限，512..=4096 的命令行会被
/// 内核放行、随后在 loader 被 E2BIG 拒绝）。契约见 docs/abi/syscall-abi.md §4。
/// 超出的命令行在 exec 时即以 E2BIG 拒绝，故用户态看到的串长必 ≤ 本值；
/// 此常量用作防御性扫描边界（用户态与内核分属两个仓，无法共享字面量，故以文档 + 测试互锚）。
pub const MAX_CMDLINE_BYTES: usize = 4096;

/// 词分隔符：空格与制表符（内核不定义分隔符语义，此处是用户态约定）。
#[inline]
fn is_sep(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// 从入口参数块取出整条命令行（不含 NUL 终止符）。
///
/// 语义（docs/abi/syscall-abi.md §4，不是 POSIX argv）：
/// - argc < 1，或 argv / argv[0] 为空 → None（无命令行）；
/// - 否则返回 argv[0] 指向的整条字符串——不含程序名，且未拆词。
///
/// # Safety
///
/// argc/argv 必须来自 _start 传入的入口参数块（内核 loader 按 §4 放置的
/// 那一份），且其指向的内存在返回值的生命周期 'a 内保持有效。生命周期由
/// 调用方选定——正常用法是在 user_main 内取用（内存位于进程初始栈，进程
/// 存活期内始终有效）。
///
/// 契约被破坏时（预算内没有 NUL 终止符）返回 None：绝不按预算截断后把
/// 裁剪过的串当成完整命令行交付（S09 错误优于伪造）。
pub unsafe fn cmdline<'a>(argc: isize, argv: *const *const u8) -> Option<&'a [u8]> {
    if argc < 1 || argv.is_null() {
        return None;
    }
    let p = unsafe { *argv };
    if p.is_null() {
        return None;
    }
    // 扫描上界必须包含终止符可能所在的最后一个字节：合法命令行最长
    // MAX_CMDLINE_BYTES 字节，其 NUL 落在偏移 MAX_CMDLINE_BYTES 处。
    let mut l = 0usize;
    while l <= MAX_CMDLINE_BYTES {
        if unsafe { *p.add(l) } == 0 {
            return Some(unsafe { core::slice::from_raw_parts(p, l) });
        }
        l += 1;
    }
    None
}

/// 按空白（空格 / 制表符）切词。纯函数，无 unsafe。
///
/// 连续空白视为一个分隔符；首尾空白忽略；不产生空词。字节语义（S02：
/// 不假设编码）。
pub fn split_words(line: &[u8]) -> Words<'_> {
    Words { rest: line }
}

/// 把 line 的词收集进固定容量 out，返回词数；放不下时返回 None。
///
/// 不静默丢弃多余词：截断会让用户以为「参数生效了」而实际没有（S09）。
pub fn words_into<'a>(line: &'a [u8], out: &mut [&'a [u8]]) -> Option<usize> {
    let mut n = 0usize;
    for w in split_words(line) {
        if n >= out.len() {
            return None;
        }
        out[n] = w;
        n += 1;
    }
    Some(n)
}

/// 一步取词：cmdline(argc, argv) 后 split_words；无命令行时为空迭代器。
///
/// # Safety
///
/// 同 cmdline。
pub unsafe fn args<'a>(argc: isize, argv: *const *const u8) -> Words<'a> {
    match unsafe { cmdline(argc, argv) } {
        Some(line) => split_words(line),
        None => split_words(b""),
    }
}

/// 空白分隔的词迭代器（见 split_words）。
#[derive(Debug, Clone, Copy)]
pub struct Words<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Words<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        let s = self.rest;
        let mut i = 0usize;
        while i < s.len() && is_sep(s[i]) {
            i += 1;
        }
        let s = &s[i..];
        if s.is_empty() {
            self.rest = s;
            return None;
        }
        let mut j = 0usize;
        while j < s.len() && !is_sep(s[j]) {
            j += 1;
        }
        self.rest = &s[j..];
        Some(&s[..j])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    // ---- cmdline：整条命令行 ----

    #[test]
    fn cmdline_none_when_argc_zero() {
        assert!(unsafe { cmdline(0, core::ptr::null()) }.is_none());
        assert!(unsafe { cmdline(-1, core::ptr::null()) }.is_none());
    }

    #[test]
    fn cmdline_none_when_argv0_null() {
        let argv = [core::ptr::null::<u8>()];
        assert!(unsafe { cmdline(1, argv.as_ptr()) }.is_none());
    }

    #[test]
    fn cmdline_reads_whole_string_without_terminator() {
        let buf = b"hello world\0";
        let argv = [buf.as_ptr()];
        let got = unsafe { cmdline(1, argv.as_ptr()) }.expect("整条命令行");
        assert_eq!(got, b"hello world");
    }

    #[test]
    fn cmdline_of_empty_string_is_some_empty() {
        let buf = b"\0";
        let argv = [buf.as_ptr()];
        let got = unsafe { cmdline(1, argv.as_ptr()) }.expect("空命令行仍是有效命令行");
        assert!(got.is_empty());
    }

    /// S02：不假设编码——非 UTF-8 字节必须原样透出。
    #[test]
    fn cmdline_preserves_non_utf8_bytes() {
        let buf = b"\xff\xfe \x80\0";
        let argv = [buf.as_ptr()];
        let got = unsafe { cmdline(1, argv.as_ptr()) }.expect("原始字节");
        assert_eq!(got, b"\xff\xfe \x80");
    }

    /// 内核允许的最大命令行：STR_OFF-1 = 511 字节，NUL 落在偏移 511。
    /// 边界必须被**接受**（差一即把合法输入拒之门外）。
    #[test]
    fn cmdline_accepts_max_budget_with_terminator() {
        let mut buf = vec![b'a'; MAX_CMDLINE_BYTES + 2];
        buf[MAX_CMDLINE_BYTES] = 0;
        let argv = [buf.as_ptr()];
        let got = unsafe { cmdline(1, argv.as_ptr()) }.expect("511 字节命令行应被接受");
        assert_eq!(got.len(), MAX_CMDLINE_BYTES);
    }

    /// 预算内没有 NUL 终止符 = 契约被破坏：**拒绝**，绝不按预算截断后
    /// 把裁剪过的串当成完整命令行交付（S09 错误优于伪造）。
    #[test]
    fn cmdline_refuses_missing_terminator_within_budget() {
        let buf = vec![b'a'; MAX_CMDLINE_BYTES + 2];
        let argv = [buf.as_ptr()];
        assert!(unsafe { cmdline(1, argv.as_ptr()) }.is_none());
    }

    // ---- split_words：按空白切词（纯函数，无 unsafe） ----

    #[test]
    fn words_split_on_space_and_tab() {
        let got: Vec<&[u8]> = split_words(b"  hello\t world  ").collect();
        assert_eq!(got, vec![&b"hello"[..], &b"world"[..]]);
    }

    #[test]
    fn words_of_blank_input_are_empty() {
        assert_eq!(split_words(b"").count(), 0);
        assert_eq!(split_words(b"   \t  ").count(), 0);
    }

    #[test]
    fn words_do_not_emit_empty_pieces() {
        // 连续空白视为一个分隔符；首尾空白忽略。
        let got: Vec<&[u8]> = split_words(b"a\t\t b").collect();
        assert_eq!(got, vec![&b"a"[..], &b"b"[..]]);
    }

    #[test]
    fn words_preserve_non_utf8_pieces() {
        let got: Vec<&[u8]> = split_words(b"\xff\xfe \x80").collect();
        assert_eq!(got, vec![&b"\xff\xfe"[..], &b"\x80"[..]]);
    }

    // ---- words_into：固定容量收集（不静默丢弃） ----

    #[test]
    fn words_into_collects_up_to_capacity() {
        let mut out: [&[u8]; 2] = [b""; 2];
        assert_eq!(words_into(b"a b", &mut out), Some(2));
        assert_eq!(out[0], b"a");
        assert_eq!(out[1], b"b");
    }

    #[test]
    fn words_into_refuses_overflow_instead_of_dropping() {
        let mut out: [&[u8]; 2] = [b""; 2];
        // 第三个词放不下：如实返回 None，让调用方报错——
        // 静默丢弃会让用户以为「参数生效了」而实际没有。
        assert_eq!(words_into(b"a b c", &mut out), None);
    }

    // ---- args：整条命令行 → 词迭代器（一步到位） ----

    #[test]
    fn args_empty_when_no_cmdline() {
        assert_eq!(unsafe { args(0, core::ptr::null()) }.count(), 0);
    }

    #[test]
    fn args_yields_words_from_raw_block() {
        let buf = b"a\tb  c\0";
        let argv = [buf.as_ptr()];
        let got: Vec<&[u8]> = unsafe { args(1, argv.as_ptr()) }.collect();
        assert_eq!(got, vec![&b"a"[..], &b"b"[..], &b"c"[..]]);
    }
}
