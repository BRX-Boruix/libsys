//! 用户程序入口 `_start`（汇编约定）。
//!
//! 由 libsys 统一提供，用户程序只需导出 `user_main`。`_start` 负责：
//! 1. 从栈上取 argc/argv（loader 已放好：`[rsp]=argc`、`[rsp+8]=argv`）；
//! 2. 对齐栈到 16 字节（System V AMD64 ABI，为 SSE 铺路）；
//! 3. 调用 `user_main`，把其返回值作为 exit code 交给 `exit`。
//!
//! # 参数语义（**不是 POSIX argv**）
//!
//! 入口栈参数块的契约见 `docs/abi/syscall-abi.md` §4；其布局的**单点定义与
//! 行为锚点**在 `kernel/crates/loader/src/lib.rs` 的 `raw::entry_block()`
//! （host 单测 `tests::entry_block_*` 锚定，文档漂移或实现回退在此先红）。
//!
//! 因此 `user_main` 收到的是：
//! - `argc`：有命令行时恒为 **1**（无命令行时 0）；
//! - `argv[0]`：指向**整条命令行字符串**（NUL 结尾），**不是程序名**——
//!   经 shell 派生时该串只含参数（shell 已剥首词），且**内核未拆词**；
//! - `argv[1]`：NULL 终结。
//!
//! **拆词是用户程序的职责**：需要多参数的程序自行按空白切分 `argv[0]`
//! （参照 `cowsay` 的 `split_args`、`userdrv` 的 `parse_argv`）。

#[cfg(feature = "crt")]
use core::arch::global_asm;

// 用户程序提供的入口函数（每个用户程序必须导出该符号）。
// 返回 `i32` 作为进程退出码。
//
// **feature `crt` 门禁**：`.so` 是库，不该带进程入口——而这里的引用会在 `.so` 里留下
// `UND user_main`（实测：rtld 的急切符号解析因此失败）。
#[cfg(feature = "crt")]
unsafe extern "C" {
    fn user_main(argc: isize, argv: *const *const u8) -> i32;
}

/// 供 `_start` 汇编调用的 exit 桥接（`call` 后不返回）。
#[cfg(feature = "crt")]
#[unsafe(no_mangle)]
extern "C" fn __libsys_exit(code: u64) -> ! {
    crate::process::exit(code as i32);
}

#[cfg(all(
    feature = "crt",
    not(test),
    any(target_os = "none", target_os = "boruix")
))]
global_asm!(
    r#"
.section .text
// **弱符号**（3P1-2）：C 程序经 sysroot 链接 `crt0.o`，它提供同名 `_start`（强定义）
// 调用 `main`。此处若为强定义，两者会在 `libc.a`（经 libsys）与 `crt0.o` 之间
// 报 duplicate symbol（实测）。弱定义让强定义胜出；而 Rust 程序无竞争者时，
// 链接器仍会为入口符号从归档中取出本对象。`.weak` 已含全局可见性，
// 不可再写 `.global _start`（实测：会把它改回 STB_GLOBAL 并报错）。
.weak _start
.type _start, @function
_start:
    xor rbp, rbp            # 标记栈帧底部（回溯终止）
    mov rdi, [rsp]          # argc
    lea rsi, [rsp + 8]      # argv
    and rsp, -16            # 对齐栈到 16 字节
    call {user_main}
    mov rdi, rax            # 返回值作为 exit code
    call {sys_exit}
    ud2                     # 不应到达
.size _start, . - _start
"#,
    user_main = sym user_main,
    sys_exit = sym __libsys_exit,
);
