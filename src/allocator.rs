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
    if actual_grow >= 32 {
        unsafe {
            heap.add_to_heap(start, end);
        }
        true
    } else {
        false
    }
}
