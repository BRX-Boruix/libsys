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

/// `next_device_event()` -> `Option<DeviceEventInfo>`：消费下一条硬件拓扑事件。
///
/// 无待消费事件返回 `None`（内核返回 0 空）。事件由内核 `driver::event` 环形
/// 日志发布（DeviceArrived / DeviceDeparted），本封装把 JSON 投影为结构化结果。
pub fn next_device_event() -> Result<Option<DeviceEventInfo>, Error> {
    let mut buf = [0u8; 512];
    let n = crate::syscall::call(
        SYS_DRIVER_EVENT_NEXT,
        [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0],
    )? as usize;
    if n == 0 {
        return Ok(None);
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
    Ok(Some(ev))
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
