//! 音频管道消费者 API（plan_audio_vfs.md 批次二，AUDIO 域 0xA0）。
//!
//! **与 VFS 的分工**：读写 PCM 走 VFS 路径——
//! `open("/devices/audio/dsp")` + `read`/`write`。本模块只提供 VFS 无法表达的
//! **流控**动词：附加/注销消费者、提交消费、显式取帧。
//!
//! **两阶段消费语义**（plan §3.2）——这是音频相对普通文件的关键差异：
//!
//! 1. [`fetch`] 把 PCM 拷到调用方缓冲，**不推进读指针**；
//! 2. 调用方把数据喂给硬件并**确认播完**后，调 [`commit`] 推进指针。
//!
//! 为何要两阶段：数据在"取出"与"播完"之间存在于硬件 FIFO 中。若取走即推进
//! 指针，一旦硬件欠载或进程崩溃，那段数据就永久丢失且**无迹可查**。两阶段让
//! "尚未确认播放"的数据留存在 ring 中，消费方才敢推进。
//!
//! **独占消费者**：同一时刻只允许一个进程附加（音频设备天然独占）。
//! 第二个 `attach` 得到 `EBUSY`——这是**结构性**占用，重试不会成功。

use crate::error::Error;
use crate::nr::{SYS_AUDIO_ATTACH, SYS_AUDIO_COMMIT, SYS_AUDIO_DETACH, SYS_AUDIO_FETCH};

/// 把当前进程注册为该音频节点的**独占**消费者。
///
/// 已有消费者（含本进程重复调用）→ `Error::Busy`（EBUSY）。
/// 重复调用**不做幂等**：它意味着调用方状态机有误，静默成功会掩盖该错误。
pub fn attach() -> Result<(), Error> {
    crate::syscall::call(SYS_AUDIO_ATTACH, [0, 0, 0, 0, 0, 0])?;
    Ok(())
}

/// 注销消费者。**仅属主可注销**；非属主 → `Error::PermissionDenied`。
///
/// 进程退出时内核会自动回收（S18），故本函数是**显式**提前释放——
/// 用于"播完即让位"的场景，不调用也不会泄漏。
pub fn detach() -> Result<(), Error> {
    crate::syscall::call(SYS_AUDIO_DETACH, [0, 0, 0, 0, 0, 0])?;
    Ok(())
}

/// 取 PCM 数据到 `buf`，返回实际字节数。**不推进读指针**（须 [`commit`]）。
///
/// - 无消费者 → `Error::NotSupported`（诚实性红线：绝不接受后丢弃）；
/// - 无数据且已附加 → **阻塞等待**（有限超时），唤醒后返回已到数据；
/// - 超时仍无数据 → `Error::WouldBlock`（EAGAIN），调用方自行决定重试。
///
/// **超时有限**（非无限）：音频前端若无人推进，无限等待会让调用者永久挂死。
pub fn fetch(buf: &mut [u8]) -> Result<usize, Error> {
    let n = crate::syscall::call(
        SYS_AUDIO_FETCH,
        [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0],
    )?;
    Ok(n as usize)
}

/// 提交已消费（且已确认播放/已拷贝完）的 `n` 字节，推进读指针释放 ring 空间。
///
/// 越界（`n` 超过当前未提交水位）→ `Error::InvalidParam`，**绝不静默截断**：
/// 静默截断会让调用方以为提交成功，实际数据仍占着空间且下次取到重复数据。
/// 非属主调用 → `Error::PermissionDenied`。
pub fn commit(n: usize) -> Result<(), Error> {
    crate::syscall::call(SYS_AUDIO_COMMIT, [n as u64, 0, 0, 0, 0, 0])?;
    Ok(())
}
