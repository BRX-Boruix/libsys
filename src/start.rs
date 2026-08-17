//! 用户程序入口 `_start`（汇编约定）。
//!
//! 由 libsys 统一提供，用户程序只需导出 `user_main`。`_start` 负责：
//! 1. 从栈上取 argc/argv（内核 `elf.rs` 已放好：`[rsp]=argc`、`[rsp+8]=argv`）；
//! 2. 对齐栈到 16 字节（System V AMD64 ABI，为 SSE 铺路）；
//! 3. 调用 `user_main`，把其返回值作为 exit code 交给 `exit`。

use core::arch::global_asm;

// 用户程序提供的入口函数（每个用户程序必须导出该符号）。
// 返回 `i32` 作为进程退出码。
unsafe extern "C" {
    fn user_main(argc: isize, argv: *const *const u8) -> i32;
}

/// 供 `_start` 汇编调用的 exit 桥接（`call` 后不返回）。
#[unsafe(no_mangle)]
extern "C" fn __libsys_exit(code: u64) -> ! {
    crate::process::exit(code as i32);
}

global_asm!(
    r#"
.section .text
.global _start
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
