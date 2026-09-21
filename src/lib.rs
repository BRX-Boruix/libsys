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
mod mem;
mod object;
mod pipe;
mod power;
mod driver;
pub mod audio;
mod process;
mod random;
mod start;
mod sync;
mod system;
mod thread;
mod time;
mod volume;

pub use error::Error;
pub use io::{
    DirEntry, GATE_SYSTEM_BIT, OpenFlags, Permissions, STDERR, STDIN, STDOUT, StatInfo, chdir,
    chmod, chown, close, dup2, fstat, getcwd, mkdir, open, pread, pwrite, read, read_dir,
    read_to_end, rename, stat, unlink, write,
};
// A2-6 / ADR-040 §3.5.1 G4：显式 ACE 通道的用户态封装与 wire 镜像。
pub use io::{
    aces_count, get_aces, set_aces, AceWire, ACE_PERM_EXECUTE, ACE_PERM_READ, ACE_PERM_WRITE,
    ACE_PRINCIPAL_NAMED_GID, ACE_PRINCIPAL_NAMED_UID, ACE_PRINCIPAL_OTHER, ACE_PRINCIPAL_OWNER,
    ACE_WIRE_MAX, ACE_WIRE_SIZE,
};
pub use mem::{brk, mmap, munmap};
pub use pipe::pipe_create;
pub use power::{power_off, reboot};
pub use process::{
    PsEntry, WAIT_ANY, WaitResult, derive, derive_inherit, exec, exec_path, exit,
    getpid, gettid, kill, ps, ps_list, waitpid_any, yield_now,
};
// A2-1 / ADR-040 §3.5 G1：身份查询与变更。**此前未在 crate 根重导出**——A2-1 落地时
// 只定义了 `process::identity_*`，`process` 模块私有，故用户程序实际**无法调用**它们
// （真实用户态 E2E `trave2e` 编译时暴露：`no identity_set in the root`）。此为可达性
// 缺口，非新增功能；一并补齐 `IdentityInfo` 的根重导出。
pub use process::{IdentityInfo, identity_query, identity_set};
// A2-4 / ADR-040 §3.5 G1：补充组变更。与上面同一类**可达性缺口**——`groups_set`/
// `groups_clear` 在 A2-4 已定义于 `process`（私有模块），但从未在 crate 根重导出，
// 故用户程序实际无法调用。A2-7 的 `login` 需要"降权前先设组"，编译时暴露了该缺口。
// 一并补齐 `GroupsInfo` 与两个子动作常量的根重导出（常量对调用方判断语义必需）。
pub use process::{GroupsInfo, groups_clear, groups_set};
pub use random::bytes as random_bytes;

pub use system::info;
pub use thread::{set_fs_base, thread_exit, thread_join, thread_spawn, thread_spawn_with_starter};
pub use time::{now, read_wall_clock, sleep, WallClock};

pub use object::{Stream, Sync, Task, Vfs};

pub use sync::{sync_create, sync_delete, sync_wait, sync_wake};

pub use volume::{DeviceEventInfo, ProbeStatus, device_probe, next_device_event, next_device_event_wait, volume_list, volume_mount, volume_unmount};

pub use driver::{
    driver_claim, driver_dma_alloc, driver_dma_free, driver_dma_phys, driver_irq_wait,
    driver_query, driver_register, driver_unregister,
};

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
    exit(101)
}
