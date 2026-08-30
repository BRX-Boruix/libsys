//! VOLUME (0x60) 域 与 DEVICE 事件通道 系统调用薄封装（ADR-030 §决策6 / P2-2）。
//!
//! 供用户态 `volumed` 守护进程订阅内核块设备事件并执行卷挂载/卸载编排。
//! 每个调用与 `kernel/syscall.rs` 侧同名实现逐一对应（双侧 ABI 对齐）。

use crate::error::Error;
use crate::nr::*;
use alloc::string::String;
use alloc::vec::Vec;

/// `volume_mount(dev_name) -> String`：把指定块设备分区挂到 `/volumes/{label}`，
/// 返回**真实挂载路径**（含同名自增消解后缀，如 `/volumes/X-2`）。
///
/// 命名与冲突消解由内核 `mount_device_volume` 决定（有卷标用卷标，无卷标用
/// `storage-{ShortUUID}`）。`dev_name` 为 `/devices/disks/...` 下的设备名。
pub fn volume_mount(dev_name: &str) -> Result<String, Error> {
    let mut buf = [0u8; 256];
    if dev_name.len() >= buf.len() {
        return Err(Error::OutOfRange);
    }
    buf[..dev_name.len()].copy_from_slice(dev_name.as_bytes());
    buf[dev_name.len()] = 0;
    let mut out = [0u8; 256];
    let n = crate::syscall::call(
        SYS_VOLUME_MOUNT,
        [buf.as_ptr() as u64, out.as_mut_ptr() as u64, out.len() as u64, 0, 0, 0],
    )? as usize;
    // 边界与内核一致（V4）：内核 `sys_volume_mount` 以 `MAX_MOUNT_PATH_BYTES=255`
    // 拒绝 `len >= 255`（即最多回传 254 字节），故 `n >= out.len()`（n≥256）在此
    // 永不误报已成功的挂载——挂载已生效却报失败的边界失配已消除。
    if n >= out.len() {
        return Err(Error::OutOfRange);
    }
    let s = core::str::from_utf8(&out[..n]).map_err(|_| Error::InvalidParam)?;
    Ok(String::from(s))
}

/// `volume_unmount(path) -> ()`：卸载指定挂载点（`/volumes/...` 绝对路径）。
pub fn volume_unmount(path: &str) -> Result<(), Error> {
    let mut buf = [0u8; 256];
    if path.len() >= buf.len() {
        return Err(Error::OutOfRange);
    }
    buf[..path.len()].copy_from_slice(path.as_bytes());
    buf[path.len()] = 0;
    crate::syscall::call(SYS_VOLUME_UNMOUNT, [buf.as_ptr() as u64, 0, 0, 0, 0, 0]).map(|_| ())
}

/// `volume_list()` -> JSON 文本：列出已挂载卷（`[{"path":"/volumes/..."}]`）。
pub fn volume_list() -> Result<Vec<u8>, Error> {
    let mut buf = [0u8; 512];
    let n = crate::syscall::call(SYS_VOLUME_LIST, [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0])? as usize;
    // 防御性边界检查（V6，S26）：内核承诺 n ≤ cap，但越界切片会 panic——与
    // 同文件 volume_mount/volume_unmount 的守卫风格一致，宁报错不 panic。
    if n > buf.len() {
        return Err(Error::OutOfRange);
    }
    Ok(buf[..n].to_vec())
}

/// `next_device_event()` -> `Option<DeviceEventInfo>`：**非阻塞**消费下一条硬件
/// 拓扑事件。无待消费事件返回 `None`（内核返回 0 空）。
///
/// 事件由内核 `driver::event` 环形日志发布（DeviceArrived / DeviceDeparted），
/// 本封装把 JSON 投影为结构化结果。
pub fn next_device_event() -> Result<Option<DeviceEventInfo>, Error> {
    next_device_event_wait(0)
}

/// `next_device_event_wait(timeout_ns)` -> `Option<DeviceEventInfo>`：阻塞等待
/// 下一条硬件拓扑事件，最多等 `timeout_ns`。
///
/// interrupt-to-futex（ADR-030 §决策3）：内核在事件队列空时挂起本进程（`Switched`
/// 语义），事件到达经 `publish_event` → `wake_event` 唤醒，超时经
/// `wake_event_timeout` 唤醒；进程回归用户态时内核把保存帧 rax 预置结果——
/// 事件唤醒置 `-EAGAIN` 哨兵（本封装识别后**重试**取事件）、超时置 `0`（返回
/// `None`，volumed 据此做周期对账）。`timeout_ns = 0` 退化为非阻塞
/// （同 [`next_device_event`]）。全程不忙转、不轮询。
pub fn next_device_event_wait(timeout_ns: u64) -> Result<Option<DeviceEventInfo>, Error> {
    loop {
        let mut buf = [0u8; 512];
        let n = crate::syscall::call(
            SYS_DRIVER_EVENT_NEXT,
            [buf.as_mut_ptr() as u64, buf.len() as u64, timeout_ns, 0, 0, 0],
        );
        let n = match n {
            // -EAGAIN 哨兵：曾阻塞、请重试（volumed 单消费者，重试即继续等）。
            Err(Error::WouldBlock) => continue,
            other => other? as usize,
        };
        if n == 0 {
            return Ok(None);
        }
        if n > buf.len() {
            return Err(Error::OutOfRange);
        }
        let text = core::str::from_utf8(&buf[..n]).map_err(|_| Error::InvalidParam)?;
        let parsed = crate::json::JsonParser::new(text).parse().map_err(|_| Error::Io)?;
        let mut ev = DeviceEventInfo {
            event: String::new(),
            kind: String::new(),
            name: String::new(),
            volatile: false,
        };
        if let crate::json::JsonValue::Object(fields) = parsed {
            for (k, v) in fields {
                match (k.as_str(), v) {
                    ("event", crate::json::JsonValue::String(s)) => ev.event = s,
                    ("kind", crate::json::JsonValue::String(s)) => ev.kind = s,
                    ("name", crate::json::JsonValue::String(s)) => ev.name = s,
                    ("volatile", crate::json::JsonValue::Bool(b)) => ev.volatile = b,
                    _ => {}
                }
            }
        }
        return Ok(Some(ev));
    }
}

/// 一条硬件拓扑事件的结构化视图（来自内核 JSON）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceEventInfo {
    /// `"arrived"` / `"departed"`。
    pub event: String,
    /// 设备类别（`"block"` 等）。
    pub kind: String,
    /// 设备名（挂载/卸载决策用）。
    pub name: String,
    /// 数据易失性披露（持久块设备为 `false`）。
    pub volatile: bool,
}

/// 块设备缓存穿透探测读的结果（对应内核 `driver::hub::ProbeStatus`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    /// 探测读成功，设备可服务。
    Alive,
    /// 探测读失败：设备已消失，内核已发布 `DeviceDeparted`。
    Gone,
    /// 设备表中无此名或实例为 None。
    NotFound,
    /// 设备存在但非 IO 设备，无可探测。
    NotIo,
}

/// `device_probe(dev_name)` -> `ProbeStatus`：对指定块设备做一次缓存穿透探测读
/// （绕过 VFS 页缓存，直接触达底层驱动真实访问设备）。
///
/// 若设备已消失（拔盘/后端移除），驱动在 `read_at` 内部经 `is_device_gone` +
/// `notify_device_gone` 发布 `DeviceDeparted`（ADR-030 热插拔闭环）。volumed
/// 低频对账用它兜底发现"拔除但无事件"的空闲卷——比高频周期对账更低频，且
/// 只触达真实设备、不扫缓存。
pub fn device_probe(dev_name: &str) -> Result<ProbeStatus, Error> {
    let mut buf = [0u8; 256];
    if dev_name.len() >= buf.len() {
        return Err(Error::OutOfRange);
    }
    buf[..dev_name.len()].copy_from_slice(dev_name.as_bytes());
    buf[dev_name.len()] = 0;
    let rax = crate::syscall::call(SYS_DEVICE_PROBE, [buf.as_ptr() as u64, 0, 0, 0, 0, 0])?;
    Ok(match rax {
        0 => ProbeStatus::Alive,
        1 => ProbeStatus::Gone,
        2 => ProbeStatus::NotFound,
        _ => ProbeStatus::NotIo,
    })
}
