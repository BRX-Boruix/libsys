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

/// `memcpy(dest, src, n)`：非重叠拷贝（Rust `copy_from_slice` 走此路径）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
        dest
    }
}

/// `memmove(dest, src, n)`：支持重叠的拷贝（`copy` 可能走此路径）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        if (dest as usize) < (src as usize) {
            let mut i = 0;
            while i < n {
                *dest.add(i) = *src.add(i);
                i += 1;
            }
        } else {
            let mut i = n;
            while i > 0 {
                i -= 1;
                *dest.add(i) = *src.add(i);
            }
        }
        dest
    }
}

/// `memset(dest, c, n)`：按字节填充（`write_bytes` 走此路径）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    unsafe {
        let c = (c & 0xff) as u8;
        let mut i = 0;
        while i < n {
            *dest.add(i) = c;
            i += 1;
        }
        dest
    }
}

/// `memcmp(a, b, n)`：逐字节比较，返回首处差值的符号（相等返回 0）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    unsafe {
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
