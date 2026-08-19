//! BORUIX 系统调用封装层（libsys）。
//!
//! 用户态程序与内核之间的 ABI 边界（ADR-002/003/006）。提供：
//! - [`nr`]：与内核对齐的系统调用号；
//! - [`syscall`]：底层 `int 0x80` 调用 + 错误解包；
//! - [`error`]：Rust 风格错误码（与内核 ADR-010 对齐）；
//! - 高层薄封装：`write`/`exit`/`now`/`sleep`/`mmap`/`brk`/`info`；
//! - `_start`：用户程序入口（汇编约定，调用用户导出的 `user_main`）。
//!
//! # 用户程序约定（ADR-003 纯 spawn，无 fork）
//!
//! ```no_run
//! #![no_std]
//! #![no_main]
//!
//! use libsys::write;
//!
//! #[unsafe(no_mangle)]
//! pub extern "C" fn user_main(_argc: isize, _argv: *const *const u8) -> i32 {
//!     let _ = write(1, b"hello from userspace\n");
//!     0
//! }
//! ```

#![no_std]

pub mod error;
pub mod nr;
pub mod signal;
pub mod syscall;

// 裸机用户程序所需的 `memcpy`/`memset`/`memmove`/`memcmp` 替身
// （`x86_64-unknown-none` 不自动链接 compiler-builtins）。非 pub 即可，符号经
// `#[no_mangle]` 进入最终二进制，被 `copy_from_slice` 等 lowering 出的调用引用。
mod allocator;
mod builtins;

mod io;
mod ipc;
mod mem;
mod process;
mod start;
mod system;
mod time;

pub use error::Error;
pub use io::{read, write, STDERR, STDIN, STDOUT};
pub use ipc::{pipe_close, pipe_create, pipe_read, pipe_write, shm_create, shm_map, shm_unmap};
pub use mem::{brk, mmap};
pub use process::{exec, exit, kill, ps, PsEntry, yield_now};
pub use system::info;
pub use time::{now, sleep};

use core::panic::PanicInfo;

/// 用户态 panic 处理：打印提示并退出（code=101）。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let _ = write(1, b"\n[libsys] userspace panic\n");
    exit(101);
}
