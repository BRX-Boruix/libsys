# BORUIX libsys

系统调用封装层：用户态程序与内核之间的 ABI 边界（ADR-002/003/006）。

## 职责
- 定义系统调用号（`nr.rs`，与内核 `kernel/src/syscall.rs` 对齐）
- 提供底层 `int 0x80` 调用 + Rust 风格 `Result<T, Error>` 错误解包（`syscall.rs`）
- 提供高层薄封装：`write` / `exit` / `now` / `sleep` / `mmap` / `brk` / `info`
- 提供用户程序入口 `_start`（`start.rs` 汇编约定）

## 依赖关系
```
libsys  ←  (无依赖)
  ▲
libc   ←  libsys
```
内核在 `kernel` 侧实现与 libsys 约定一致的 syscall ABI。

## 用户程序约定（ADR-003 纯 spawn，无 fork）

```rust
#![no_std]
#![no_main]

use libsys::{write, STDOUT};

#[unsafe(no_mangle)]
pub extern "C" fn user_main(_argc: isize, _argv: *const *const u8) -> i32 {
    let _ = write(STDOUT, b"hello from userspace\n");
    0
}
```

libsys 提供 `_start`：栈上取 argc/argv → 对齐栈 → 调 `user_main` → 其返回值作为
exit code。`user_main` 是用户程序唯一需要导出的符号。

## syscall ABI
- 编码 `(domain << 8) | op`，`int 0x80` 触发；
- `rax = 系统调用号`，`rdi/rsi/rdx/r10/r8/r9 = 参数`；
- 返回 `rax` 全 64 位；错误时 `bit63` 置位（`-errno` 补码），libsys 解包为 `Error`。

## 模块
- `src/syscall.rs` — `int 0x80` 入口与参数约定 + `Result` 解包
- `src/nr.rs` — 系统调用号定义
- `src/error.rs` — 与内核 ADR-010 对齐的错误码
- `src/io.rs` / `src/process.rs` / `src/time.rs` / `src/mem.rs` / `src/system.rs` — 高层薄封装
- `src/start.rs` — `_start` 汇编入口
