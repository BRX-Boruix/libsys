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
//! ```ignore
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

extern crate alloc;

pub mod error;
pub mod json;
pub mod nr;
pub mod signal;
pub mod syscall;

#[cfg(all(not(test), target_os = "none"))]
mod allocator;
#[cfg(all(not(test), target_os = "none"))]
mod builtins;

mod io;
mod ipc;
mod mem;
mod process;
mod start;
mod system;
mod time;

pub use error::Error;
pub use io::{
    DirEntry, OpenFlags, Permissions, STDERR, STDIN, STDOUT, close, mkdir, open, pread, pwrite,
    read, read_dir, read_to_end, unlink, write,
};
pub use ipc::{pipe_close, pipe_create, pipe_read, pipe_write, shm_create, shm_map};
pub use mem::{brk, mmap, munmap};
pub use process::{PsEntry, exec, exec_path, exit, kill, ps, ps_list, yield_now};

pub use system::info;
pub use time::{now, sleep};

use core::panic::PanicInfo;

/// 用户态 panic 处理：打印提示并退出（code=101）。
#[cfg(all(not(test), target_os = "none"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = write(1, b"\n[libsys] userspace panic: ");
    if let Some(loc) = info.location() {
        let _ = write(1, loc.file().as_bytes());
        let _ = write(1, b":");
        let mut buf = [0u8; 10];
        let mut n = loc.line();
        let mut i = 0;
        if n == 0 {
            let _ = write(1, b"0");
        } else {
            let mut tmp = [0u8; 10];
            while n > 0 {
                tmp[i] = b'0' + (n % 10) as u8;
                n /= 10;
                i += 1;
            }
            let mut j = 0;
            while i > 0 {
                i -= 1;
                buf[j] = tmp[i];
                j += 1;
            }
            let _ = write(1, &buf[..j]);
        }
    }
    let _ = write(1, b"\n");
    let _ = exit(101);
    loop {}
}
