//! 极简 `compiler-builtins` 替身：为裸机用户程序提供 `memcpy`/`memmove`/`memset`/
//! `memcmp`。
//!
//! `x86_64-unknown-none` 的 `no_std` 目标**不会**自动链接 `compiler-builtins`，而
//! Rust 的 `copy_from_slice` / `write_bytes` / `copy` 等会 lowering 成对 `memcpy` /
//! `memset` / `memmove` 的调用。若这些符号缺失，链接器会把它们解析成地址 0，运行到
//! 第一个 `copy_from_slice` 时经 `jmp *(rip)` 间接跳转读 NULL，触发 page fault
//!（`cr2=0`，用户态读访问）——这正是 `export` 命令崩溃的根因（`env_set` 内部
//! `copy_from_slice`）。此处用 SysV C ABI 手动实现兜底，无需引入外部 crate。
//!
//! 仅依赖 `libsys` 的用户程序（init / shell）会因此自动获得这些符号。
//!
//! 签名必须与标准库 `compiler-builtins` 期望的 `c_void` 指针一致，否则触发
//! `suspicious_runtime_symbol_definitions` 警告。

use core::ffi::c_void;

/// 机器字宽度（本实现按它推进；见下面 `memcpy` 的说明）。
const W: usize = core::mem::size_of::<usize>();
/// 一次搬 4 个机器字（x86-64 上 32 字节）。
const CHUNK: usize = 4 * W;

/// `memcpy(dest, src, n)`：非重叠拷贝（Rust `copy_from_slice` 走此路径）。
///
/// ## 为什么按机器字而不是逐字节（2026-10 实测的**根因**，S06 真实链路）
///
/// 这四个函数是**整个用户态的 C ABI `memcpy`/`memset`/`memmove`/`memcmp`**
/// （libc 刻意不重复导出，见 `libc/src/string.rs` 的说明）。逐字节实现意味着**每个程序的
/// 每一次拷贝都慢 8~32 倍**。实测代价非常干净：
///
/// - 同一个 9.4 MB 的 GCC 源文件 `insn-recog.cc`：
///   clang（用 Windows CRT 的 SIMD memcpy）**20 秒**编完；
///   GCC 的 `cc1plus`（静态链本实现）**70+ 分钟**编不完，且工作集 100 分钟稳定在 506 MB
///   而 CPU 持续燃烧——正是"数据不动、只反复搬"的形状。
/// - 在 Boruix 内跑同一个 `cc1`：启动后长时间无输出，QEMU 12 秒烧 49 秒 CPU。
///
/// 改法：**每次搬 4 个机器字（32 字节）**，用 `read_unaligned`/`write_unaligned` 明确告诉
/// 编译器可以非对齐访问（x86-64 本就允许），余数按机器字、再按字节收尾。
/// **语义完全不变**：只读 `[src, src+n)`、只写 `[dest, dest+n)`，不越界、不多读。
///
/// **注意不能改用 `core::ptr::copy_nonoverlapping`**：它会被 LLVM lowering 成对
/// `memcpy` 的调用，而这里就是 `memcpy` 本体 ⇒ 无限递归。故必须显式读写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe {
        let d = dest as *mut u8;
        let s = src as *const u8;
        let mut i = 0usize;
        while i + CHUNK <= n {
            let p = s.add(i) as *const usize;
            let q = d.add(i) as *mut usize;
            core::ptr::write_unaligned(q, core::ptr::read_unaligned(p));
            core::ptr::write_unaligned(q.add(1), core::ptr::read_unaligned(p.add(1)));
            core::ptr::write_unaligned(q.add(2), core::ptr::read_unaligned(p.add(2)));
            core::ptr::write_unaligned(q.add(3), core::ptr::read_unaligned(p.add(3)));
            i += CHUNK;
        }
        while i + W <= n {
            core::ptr::write_unaligned(
                d.add(i) as *mut usize,
                core::ptr::read_unaligned(s.add(i) as *const usize),
            );
            i += W;
        }
        while i < n {
            *d.add(i) = *s.add(i);
            i += 1;
        }
        dest
    }
}

/// `memmove(dest, src, n)`：支持重叠的拷贝（`copy` 可能走此路径）。
///
/// 按机器字推进（理由见 `memcpy`），但**不能像 `memcpy` 那样一次搬 32 字节**：
/// 重叠时那会在块内覆盖**本块还没读**的源字节。**宿主对照测试实测**：n ≥ 32 且
/// `dest > src` 时逐点全错（`tools/checks/builtins_verify`，3 秒抓出，比跑一轮 QEMU 便宜
/// 三个数量级）。故这里两个方向都**一次只搬一个机器字**：
/// - `dest <= src` 从低到高：写 `d[i]` 只碰到源里 `i-(src-dest) < i` 的字节，**已读过**；
/// - `dest >  src` 从高到低：写 `d[i]` 只碰到源里 `i+(dest-src) > i` 的字节，**已读过**。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe {
        let d = dest as *mut u8;
        let s = src as *const u8;
        if (d as usize) <= (s as usize) {
            let mut i = 0usize;
            while i + W <= n {
                core::ptr::write_unaligned(
                    d.add(i) as *mut usize,
                    core::ptr::read_unaligned(s.add(i) as *const usize),
                );
                i += W;
            }
            while i < n {
                *d.add(i) = *s.add(i);
                i += 1;
            }
        } else {
            let mut i = n;
            while i >= W {
                i -= W;
                core::ptr::write_unaligned(
                    d.add(i) as *mut usize,
                    core::ptr::read_unaligned(s.add(i) as *const usize),
                );
            }
            while i > 0 {
                i -= 1;
                *d.add(i) = *s.add(i);
            }
        }
        dest as *mut c_void
    }
}

/// `memset(dest, c, n)`：填充（`write_bytes` 走此路径）。按机器字推进（理由见 `memcpy`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: i32, n: usize) -> *mut c_void {
    unsafe {
        let d = dest as *mut u8;
        let b = (c & 0xff) as u8;
        // 把字节模式广播到整个机器字：0xDD -> 0xDDDD_DDDD_DDDD_DDDD。
        let mut word = 0usize;
        let mut k = 0;
        while k < W {
            word = (word << 8) | (b as usize);
            k += 1;
        }
        let mut i = 0usize;
        while i + W <= n {
            core::ptr::write_unaligned(d.add(i) as *mut usize, word);
            i += W;
        }
        while i < n {
            *d.add(i) = b;
            i += 1;
        }
        dest as *mut c_void
    }
}

/// `memcmp(a, b, n)`：逐字节比较，返回首处差值的符号（相等返回 0）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    unsafe {
        let a = a as *const u8;
        let b = b as *const u8;
        let mut i = 0;
        while i < n {
            let va = *a.add(i);
            let vb = *b.add(i);
            if va != vb {
                return (va as i32) - (vb as i32);
            }
            i += 1;
        }
        0
    }
}
