//! DEVICE (0x50) domain user-driver (UIO) syscall thin wrappers (ADR-008 / M11).
//!
//! Lets a userspace process act as a driver for a real device: register a claim,
//! then map that device's MMIO window into this process's address space
//! (uncacheable pages) and poke the hardware registers directly from user mode -
//! the same idea as Linux Userspace I/O (UIO).
//!
//! Authorisation (KA4): registration is a unique claim (one live claim per
//! device); claim locates by uio_id and checks caller ownership. The device
//! MMIO window is kernel-registered fact (published from PCI BARs); a user
//! supplied phys/size never takes effect. On process exit/crash the kernel
//! auto-releases its claim (0 panic).
//!
//! Each call mirrors the same-named kernel/syscall.rs implementation (dual-side ABI).

use crate::error::Error;
use crate::nr::*;
use alloc::string::String;

/// Max device name (DriverHub register name) length, matching kernel uio::UIO_DEV_NAME_MAX.
const UIO_DEV_NAME_MAX: usize = 32;

/// driver_register(dev_name) -> uio_id : register this process as the driver
/// instance for dev_name and claim it. Returns uio_id (for later claim/unregister).
///
/// The device must be really registered in the kernel DriverHub (else NotFound -
/// no arbitrary-name "ghost device" claims); duplicate registration returns
/// AlreadyExists (unique claim).
pub fn driver_register(dev_name: &str) -> Result<u64, Error> {
    if dev_name.is_empty() || dev_name.len() > UIO_DEV_NAME_MAX {
        return Err(Error::InvalidParam);
    }
    let mut buf = [0u8; UIO_DEV_NAME_MAX];
    buf[..dev_name.len()].copy_from_slice(dev_name.as_bytes());
    crate::syscall::call(
        SYS_DRIVER_REGISTER,
        [buf.as_ptr() as u64, dev_name.len() as u64, 0, 0, 0, 0],
    )
}

/// driver_query(dev_name) -> String : query the device binding state, returns compact JSON.
/// Shape: {"device":"<name>","binding":"driver:<d>","uio_claimed":<bool>};
/// unknown device -> {"error":"not_found","device":"<name>"}.
pub fn driver_query(dev_name: &str) -> Result<String, Error> {
    if dev_name.is_empty() || dev_name.len() > UIO_DEV_NAME_MAX {
        return Err(Error::InvalidParam);
    }
    let mut name = [0u8; UIO_DEV_NAME_MAX];
    name[..dev_name.len()].copy_from_slice(dev_name.as_bytes());
    let mut out = [0u8; 512];
    let n = crate::syscall::call(
        SYS_DRIVER_QUERY,
        [name.as_ptr() as u64, out.as_mut_ptr() as u64, out.len() as u64, 0, 0, 0],
    )? as usize;
    if n == 0 || n > out.len() {
        return Err(Error::OutOfRange);
    }
    let s = core::str::from_utf8(&out[..n]).map_err(|_| Error::InvalidParam)?;
    Ok(String::from(s))
}

/// driver_claim(uio_id) -> user_vaddr : map the claimed device's MMIO window into
/// this process's address space (uncacheable), return the user virtual address.
/// Authorisation by uio_id + caller pid; device with no window -> NotSupported.
pub fn driver_claim(uio_id: u64) -> Result<u64, Error> {
    // Kernel ignores a2/a3 (legacy mmio_base/size - window is kernel fact); a1 = uio_id.
    crate::syscall::call(SYS_DRIVER_CLAIM, [uio_id, 0, 0, 0, 0, 0])
}

/// driver_unregister(uio_id) -> () : drop this process's driver claim, freeing the slot
/// so the device becomes claimable again. Ownership check same as claim.
pub fn driver_unregister(uio_id: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_DRIVER_UNREGISTER, [uio_id, 0, 0, 0, 0, 0]).map(|_| ())
}

/// driver_irq_wait(uio_id, timeout_ns) -> bool : block until the claimed device's
/// interrupt fires (returns true = go service the device) or the timeout elapses
/// (returns false). Lets a userspace driver be interrupt-driven instead of polling.
///
/// Only meaningful for devices with a PCI interrupt line; interrupt-less devices
/// yield Error::NotSupported. Ownership check same as claim.
pub fn driver_irq_wait(uio_id: u64, timeout_ns: u64) -> Result<bool, Error> {
    // 内核返回：1 = 中断已触发待服务；0 = 超时无中断；负值 = 错误（syscall::call
    // 已解包为 Err）。
    let r = crate::syscall::call(SYS_DRIVER_IRQ_WAIT, [uio_id, timeout_ns, 0, 0, 0, 0])?;
    Ok(r == 1)
}

/// driver_dma_alloc(bytes) -> user_vaddr (stage-2 DMA coherent buffer).
///
/// Allocate a physically-contiguous, uncacheable(PCD)-mapped RAM buffer the caller
/// can write and hand to a claimed device via its physical address (driver_dma_phys).
/// System-only. Buffer is auto-reclaimed when the process exits. bytes==0 or
/// >64MiB -> InvalidParam; physical allocation failure -> OutOfMemory.
pub fn driver_dma_alloc(bytes: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_DRIVER_DMA_ALLOC, [bytes, 0, 0, 0, 0, 0])
}

/// driver_dma_phys(vaddr) -> phys : base physical address of a DMA buffer the caller
/// owns (vaddr is its start). Used to program the device DMA descriptor. Non-DMA
/// vaddr -> NotFound.
pub fn driver_dma_phys(vaddr: u64) -> Result<u64, Error> {
    crate::syscall::call(SYS_DRIVER_DMA_PHYS, [vaddr, 0, 0, 0, 0, 0])
}

/// driver_dma_free(vaddr) -> () : release a DMA coherent buffer (unmap + return the
/// physical frames). Non-DMA vaddr -> NotFound.
pub fn driver_dma_free(vaddr: u64) -> Result<(), Error> {
    crate::syscall::call(SYS_DRIVER_DMA_FREE, [vaddr, 0, 0, 0, 0, 0]).map(|_| ())
}
