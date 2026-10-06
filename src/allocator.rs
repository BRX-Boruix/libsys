//! 用户态堆分配器（基于 `brk` 动态增长）。
//!
//! 基于 `buddy_system_allocator::Heap`：
//! - 用户态 `alloc` 首次分配或 OOM 时，调用 `brk` 系统调用向内核申请扩展堆内存。
//! - 自动注册为 `#[global_allocator]`，允许用户态程序使用 `extern crate alloc`
//!   以及 `Vec`, `String`, `Box` 等标准集合容器。

use buddy_system_allocator::Heap;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// 简单的用户态互斥锁（SpinLock）。
struct SimpleMutex<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SimpleMutex<T> {}
unsafe impl<T: Send> Send for SimpleMutex<T> {}

impl<T> SimpleMutex<T> {
    const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    fn lock(&self) -> SimpleMutexGuard<'_, T> {
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SimpleMutexGuard { mutex: self }
    }
}

struct SimpleMutexGuard<'a, T> {
    mutex: &'a SimpleMutex<T>,
}

impl<'a, T> core::ops::Deref for SimpleMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SimpleMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for SimpleMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.lock.store(false, Ordering::Release);
    }
}

/// 用户态堆结构。
struct UserHeap {
    inner: SimpleMutex<Heap<32>>,
}

unsafe impl GlobalAlloc for UserHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.inner.lock();
        if let Ok(non_null) = heap.alloc(layout) {
            return non_null.as_ptr();
        }
        // OOM：通过 brk 向内核扩充堆空间
        for _ in 0..4 {
            if grow_user_heap(&mut heap, &layout) {
                if let Ok(non_null) = heap.alloc(layout) {
                    return non_null.as_ptr();
                }
            } else {
                break;
            }
        }
        // S09 留证：panic 前在串口打一行归因信息（布局与次数），让
        // 「buddy_system_allocator panic」能定位到分配规模与来源。
        // 用 write syscall（栈字节切片，零堆分配——OOM 路径绝不能再
        // 摸堆）。失败静默（无更外层通道，S09 到顶）。
        oom_trace(layout);
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.inner.lock();
        unsafe { heap.dealloc(core::ptr::NonNull::new_unchecked(ptr), layout) };
    }
}

#[global_allocator]
static USER_HEAP_ALLOCATOR: UserHeap = UserHeap {
    inner: SimpleMutex::new(Heap::empty()),
};

// ===== TEMP-DIAG（本轮临时观测点，定位后必须删除）=====
// 目的：交替探针下，libsys 的 buddy 与 libc 的 malloc 都在推进 brk。
// 两边各打一行 (tag, cur_brk, new_brk)，用来判断区间是否真的相交。
// 零堆分配：栈缓冲 + write（与 oom_trace 同一手法）。
/// 诊断开关：**默认关闭**，由 `libc` 的 `boruix_heap_diag(1)` 打开。
///
/// 为什么不无条件打印：这两个 crate 的分配路径是**所有程序**的地基，无条件输出会污染
/// 每个程序的串口。诊断设施必须默认静默、可显式开启（本文件与 libc/malloc.rs 各有一份）。
static DIAG_ON: AtomicBool = AtomicBool::new(false);

/// 开关堆增长诊断（C 侧入口见 `libc` 的 `boruix_heap_diag`）。
pub fn heap_diag(on: bool) {
    DIAG_ON.store(on, Ordering::Relaxed);
}

/// 诊断是否已开启（供 libc 侧同开关使用）。
pub fn heap_diag_on() -> bool {
    DIAG_ON.load(Ordering::Relaxed)
}

const DIAG_HEX: &[u8; 16] = b"0123456789abcdef";

fn diag_brk(tag: u8, a: u64, b: u64) {
    if !DIAG_ON.load(Ordering::Relaxed) {
        return;
    }
    let mut buf = [0u8; 40];
    buf[0] = b'[';
    buf[1] = tag;
    buf[2] = b']';
    buf[3] = b' ';
    for i in 0..16usize {
        buf[4 + i] = DIAG_HEX[((a >> (60 - i * 4)) & 0xf) as usize];
    }
    buf[20] = b' ';
    for i in 0..16usize {
        buf[21 + i] = DIAG_HEX[((b >> (60 - i * 4)) & 0xf) as usize];
    }
    buf[37] = b'\n';
    let _ = crate::io::write(1, &buf[..38]);
}
// ===== /TEMP-DIAG =====

/// 堆增长：向内核调用 `brk` 扩展虚拟堆空间并加入 buddy 堆。
fn grow_user_heap(heap: &mut Heap<32>, layout: &Layout) -> bool {
    let Ok(cur_brk) = crate::mem::brk(0) else {
        return false;
    };
    let need = layout.size().max(4096);
    // 每次至少增长 64KB 或 2 倍所需大小，减少频繁 syscall
    let grow_size = (need * 2).max(64 * 1024);
    // 4KB 对齐
    let grow_size_aligned = (grow_size + 4095) & !4095;

    let target_brk = cur_brk + grow_size_aligned as u64;
    let Ok(new_brk) = crate::mem::brk(target_brk) else {
        return false;
    };
    if new_brk <= cur_brk {
        return false;
    }
    let actual_grow = (new_brk - cur_brk) as usize;
    // buddy_system_allocator 要求 start 对齐到至少 32 字节且大小大于 0
    let start = cur_brk as usize;
    let end = new_brk as usize;
    diag_brk(b'L', cur_brk, new_brk); // TEMP-DIAG
    if actual_grow >= 32 {
        unsafe {
            heap.add_to_heap(start, end);
        }
        true
    } else {
        false
    }
}

/// OOM 归因行（S09）：`[libsys] heap OOM size=N align=N`——只在分配
/// 器放弃时调用，全局计数防重复刷屏。零堆分配（栈缓冲 + write）。
fn oom_trace(layout: Layout) {
    use core::sync::atomic::{AtomicU8, Ordering};
    static OOM_COUNT: AtomicU8 = AtomicU8::new(0);
    if OOM_COUNT.fetch_add(1, Ordering::Relaxed) >= 8 {
        return; // 前 8 次留证后静默——洪泛下不刷屏
    }
    let mut msg = [0u8; 64];
    const PREFIX: &[u8] = b"[libsys] heap OOM size=0000000 align=00\n";
    msg[..PREFIX.len()].copy_from_slice(PREFIX);
    let size = layout.size();
    let align = layout.align();
    // 手写十进制（无堆无 format!）：size 最多 7 位、align 2 位槽
    let mut sb = [0u8; 7];
    let mut n = 0usize;
    let mut v = size;
    while v > 0 {
        sb[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
    }
    // size 槽 [22..29)：右对齐——末位放 28，向左回退
    for k in (0..n).rev() {
        msg[22 + (6 - k)] = sb[k];
    }
    let mut ab = [0u8; 2];
    let mut n2 = 0usize;
    let mut v2 = align;
    while v2 > 0 {
        ab[n2] = b'0' + (v2 % 10) as u8;
        n2 += 1;
        v2 /= 10;
    }
    // align 槽 [36..38)：右对齐——末位放 37，向左回退
    for k in (0..n2).rev() {
        msg[36 + (1 - k)] = ab[k];
    }
    unsafe {
        let _ = crate::io::write(1, &msg);
    }
}
