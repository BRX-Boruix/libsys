//! 输入事件流转换层（I-EVENTS 阶段 2，ADR-047；键盘布局上移用户态的落实）。
//!
//! ADR-045 决策 3：事件→字节的转换（keymap、转义、Ctrl 折叠）归**用户态**。
//! 本模块是内核 KEYMAP/KEYMAP_SHIFT/decode_key 语义的**逐字节复刻**：
//! - KEYMAP/KEYMAP_SHIFT：与 arch-x86_64/src/keyboard.rs 完全一致；
//! - feed：Shift/Ctrl 修饰键状态机（内核在 IRQ1 维护，这里由事件流维护）；
//! - Ctrl+字母折叠 & 0x1F 只对字母（内核 §6.8 同款，S17 安全侧）。
//!
//! **纯函数设计（S23）**：feed 只吃一条已解析记录、吐出 0..1 段字节——宿主测试
//! 无需内核/硬件即可锁定全部语义。阶段 2 后续小点把 /devices/input/events 的
//! 字节流喂进来，产出与旧路径（键盘→字节流）**逐字节等价**的输出。
//!
//! **释放事件的用途**：修饰键状态机靠 KEY_UP 事件清位——字节流形态下释放被丢弃、
//! 「Shift 按住」只能由内核单方面折叠；事件化后这是表达力提升的根源（ADR-047 §2.2）。

use crate::error::Error;

/// 事件记录定长（ADR-047 §2.1）。
pub const EVENT_RECORD_SIZE: usize = 16;

/// 事件 kind：键按下 / 键释放（与内核 keyboard 常量镜像）。
pub const EVENT_KIND_KEY_DOWN: u8 = 1;
pub const EVENT_KIND_KEY_UP: u8 = 2;

/// flags.bit0：E0 扩展前缀。
pub const EVENT_FLAG_E0: u8 = 1 << 0;
/// flags.bit1：时间戳不可得标注（timestamp 恒 0 且带此位）。
pub const EVENT_FLAG_NO_TIME: u8 = 1 << 1;

/// 一条已解析的事件记录（ADR-047 §2.1 布局的 Rust 视图）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRecord {
    /// 1=键按下 2=键释放（3/4/5 指针类预留，本模块不产出）。
    pub kind: u8,
    /// bit0=E0 前缀；bit1=时间戳不可得。
    pub flags: u8,
    /// 键事件：scancode set 1 键码（不含 bit7）。
    pub code: u16,
    /// 键事件恒 0。
    pub value: u32,
    /// HPET ns；不可得时 0 且 flags.bit1=1。
    pub timestamp: u64,
}

/// 解析一条 16 字节记录。长度不足返回 Err(InvalidParam)（半条不是记录）。
pub fn parse_record(buf: &[u8]) -> Result<EventRecord, Error> {
    if buf.len() < EVENT_RECORD_SIZE {
        return Err(Error::InvalidParam);
    }
    let lo = u64::from_le_bytes(buf[0..8].try_into().unwrap());
    let timestamp = u64::from_le_bytes(buf[8..16].try_into().unwrap());
    Ok(EventRecord {
        kind: (lo & 0xFF) as u8,
        flags: ((lo >> 8) & 0xFF) as u8,
        code: ((lo >> 16) & 0xFFFF) as u16,
        value: ((lo >> 32) & 0xFFFF_FFFF) as u32,
        timestamp,
    })
}

/// 修饰键状态机 + 事件→字节转换器（内核 decode_key 的用户态镜像）。
#[derive(Default)]
pub struct KeymapState {
    shift: bool,
    ctrl: bool,
}

/// 转换输出：一段字节（转义序列最长 \x1b[15~ = 5 字节，8 足够）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyOut {
    buf: [u8; 8],
    len: usize,
}

impl KeyOut {
    fn one(b: u8) -> Option<KeyOut> {
        let mut k = KeyOut { buf: [0; 8], len: 1 };
        k.buf[0] = b;
        Some(k)
    }
    fn seq(s: &[u8]) -> Option<KeyOut> {
        let mut k = KeyOut { buf: [0; 8], len: s.len() };
        k.buf[..s.len()].copy_from_slice(s);
        Some(k)
    }
    /// 输出字节切片。
    pub fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

const SC_LSHIFT: u16 = 0x2A;
const SC_RSHIFT: u16 = 0x36;
const SC_LCTRL: u16 = 0x1D;

/// 扫描码 → ASCII（无 Shift）——与内核 KEYMAP 逐字节一致。
static KEYMAP: [u8; 0x80] = build_keymap();
/// 扫描码 → ASCII（Shift）——与内核 KEYMAP_SHIFT 逐字节一致。
static KEYMAP_SHIFT: [u8; 0x80] = build_keymap_shift();

const fn km(m: &mut [u8; 0x80], code: u16, ch: u8) {
    m[code as usize] = ch;
}

const fn build_keymap() -> [u8; 0x80] {
    let mut m = [0u8; 0x80];
    km(&mut m, 0x01, 27);
    km(&mut m, 0x02, b'1');
    km(&mut m, 0x03, b'2');
    km(&mut m, 0x04, b'3');
    km(&mut m, 0x05, b'4');
    km(&mut m, 0x06, b'5');
    km(&mut m, 0x07, b'6');
    km(&mut m, 0x08, b'7');
    km(&mut m, 0x09, b'8');
    km(&mut m, 0x0A, b'9');
    km(&mut m, 0x0B, b'0');
    km(&mut m, 0x0C, b'-');
    km(&mut m, 0x0D, b'=');
    km(&mut m, 0x0E, 0x7F);
    km(&mut m, 0x0F, 0x09);
    km(&mut m, 0x10, b'q');
    km(&mut m, 0x11, b'w');
    km(&mut m, 0x12, b'e');
    km(&mut m, 0x13, b'r');
    km(&mut m, 0x14, b't');
    km(&mut m, 0x15, b'y');
    km(&mut m, 0x16, b'u');
    km(&mut m, 0x17, b'i');
    km(&mut m, 0x18, b'o');
    km(&mut m, 0x19, b'p');
    km(&mut m, 0x1A, b'[');
    km(&mut m, 0x1B, b']');
    km(&mut m, 0x1C, 0x0A);
    km(&mut m, 0x1E, b'a');
    km(&mut m, 0x1F, b's');
    km(&mut m, 0x20, b'd');
    km(&mut m, 0x21, b'f');
    km(&mut m, 0x22, b'g');
    km(&mut m, 0x23, b'h');
    km(&mut m, 0x24, b'j');
    km(&mut m, 0x25, b'k');
    km(&mut m, 0x26, b'l');
    km(&mut m, 0x27, b';');
    km(&mut m, 0x28, 0x27);
    km(&mut m, 0x29, 0x60);
    km(&mut m, 0x2B, 0x5C);
    km(&mut m, 0x2C, b'z');
    km(&mut m, 0x2D, b'x');
    km(&mut m, 0x2E, b'c');
    km(&mut m, 0x2F, b'v');
    km(&mut m, 0x30, b'b');
    km(&mut m, 0x31, b'n');
    km(&mut m, 0x32, b'm');
    km(&mut m, 0x33, b',');
    km(&mut m, 0x34, b'.');
    km(&mut m, 0x35, b'/');
    km(&mut m, 0x39, b' ');
    m
}

const fn build_keymap_shift() -> [u8; 0x80] {
    let mut m = [0u8; 0x80];
    km(&mut m, 0x02, b'!');
    km(&mut m, 0x03, b'@');
    km(&mut m, 0x04, b'#');
    km(&mut m, 0x05, b'$');
    km(&mut m, 0x06, b'%');
    km(&mut m, 0x07, b'^');
    km(&mut m, 0x08, b'&');
    km(&mut m, 0x09, b'*');
    km(&mut m, 0x0A, b'(');
    km(&mut m, 0x0B, b')');
    km(&mut m, 0x0C, b'_');
    km(&mut m, 0x0D, b'+');
    km(&mut m, 0x10, b'Q');
    km(&mut m, 0x11, b'W');
    km(&mut m, 0x12, b'E');
    km(&mut m, 0x13, b'R');
    km(&mut m, 0x14, b'T');
    km(&mut m, 0x15, b'Y');
    km(&mut m, 0x16, b'U');
    km(&mut m, 0x17, b'I');
    km(&mut m, 0x18, b'O');
    km(&mut m, 0x19, b'P');
    km(&mut m, 0x1A, b'{');
    km(&mut m, 0x1B, b'}');
    km(&mut m, 0x1E, b'A');
    km(&mut m, 0x1F, b'S');
    km(&mut m, 0x20, b'D');
    km(&mut m, 0x21, b'F');
    km(&mut m, 0x22, b'G');
    km(&mut m, 0x23, b'H');
    km(&mut m, 0x24, b'J');
    km(&mut m, 0x25, b'K');
    km(&mut m, 0x26, b'L');
    km(&mut m, 0x27, b':');
    km(&mut m, 0x28, b'"');
    km(&mut m, 0x29, b'~');
    km(&mut m, 0x2B, b'|');
    km(&mut m, 0x2C, b'Z');
    km(&mut m, 0x2D, b'X');
    km(&mut m, 0x2E, b'C');
    km(&mut m, 0x2F, b'V');
    km(&mut m, 0x30, b'B');
    km(&mut m, 0x31, b'N');
    km(&mut m, 0x32, b'M');
    km(&mut m, 0x33, b'<');
    km(&mut m, 0x34, b'>');
    km(&mut m, 0x35, b'?');
    m
}

/// 喂一条事件，产出 0 或 1 段字节（语义与内核 decode_key 逐分支一致）。
///
/// - KIND 1/2 之外的 kind（指针预留类）：返回 None（本模块不认识）；
/// - Shift/Ctrl 自身的按下/释放：维护状态，不产字节；
/// - 其余键的释放：不产字节（字节流语义）；
/// - 其余按下：查表产出 ASCII/转义序列。
pub fn feed(state: &mut KeymapState, rec: &EventRecord) -> Option<KeyOut> {
    if rec.kind != EVENT_KIND_KEY_DOWN && rec.kind != EVENT_KIND_KEY_UP {
        return None;
    }
    let key_up = rec.kind == EVENT_KIND_KEY_UP;
    let e0 = rec.flags & EVENT_FLAG_E0 != 0;
    let code = rec.code;
    // 修饰键状态机（含自身事件）。
    if code == SC_LSHIFT || code == SC_RSHIFT {
        state.shift = !key_up;
        return None;
    }
    if code == SC_LCTRL {
        state.ctrl = !key_up;
        return None;
    }
    if key_up {
        return None;
    }
    if !e0 {
        let shift = state.shift;
        let ch = if shift { KEYMAP_SHIFT[code as usize] } else { KEYMAP[code as usize] };
        if ch != 0 {
            // §6.8 同款：Ctrl 只折叠字母（非字母保持原样，S17 安全侧）。
            if state.ctrl && ch.is_ascii_alphabetic() {
                return KeyOut::one(ch & 0x1F);
            }
            return KeyOut::one(ch);
        }
        // NumLock 小键盘 / 主键盘符号 / F 键（内核同表）。
        return match code {
            0x47 => KeyOut::one(b'7'),
            0x48 => KeyOut::one(b'8'),
            0x49 => KeyOut::one(b'9'),
            0x4B => KeyOut::one(b'4'),
            0x4C => KeyOut::one(b'5'),
            0x4D => KeyOut::one(b'6'),
            0x4F => KeyOut::one(b'1'),
            0x50 => KeyOut::one(b'2'),
            0x51 => KeyOut::one(b'3'),
            0x52 => KeyOut::one(b'0'),
            0x53 => KeyOut::one(b'.'),
            0x37 => KeyOut::one(b'*'),
            0x4A => KeyOut::one(b'-'),
            0x4E => KeyOut::one(b'+'),
            0x3B => KeyOut::seq(b"\x1bOP"),
            0x3C => KeyOut::seq(b"\x1bOQ"),
            0x3D => KeyOut::seq(b"\x1bOR"),
            0x3E => KeyOut::seq(b"\x1bOS"),
            0x3F => KeyOut::seq(b"\x1b[15~"),
            0x40 => KeyOut::seq(b"\x1b[17~"),
            0x41 => KeyOut::seq(b"\x1b[18~"),
            0x42 => KeyOut::seq(b"\x1b[19~"),
            0x43 => KeyOut::seq(b"\x1b[20~"),
            0x44 => KeyOut::seq(b"\x1b[21~"),
            0x57 => KeyOut::seq(b"\x1b[23~"),
            0x58 => KeyOut::seq(b"\x1b[24~"),
            _ => None,
        };
    }
    // E0 扩展键：方向键 / 编辑键 / 小键盘 Enter、'/'（内核同表）。
    match code {
        0x48 => KeyOut::seq(b"\x1b[A"),
        0x50 => KeyOut::seq(b"\x1b[B"),
        0x4B => KeyOut::seq(b"\x1b[D"),
        0x4D => KeyOut::seq(b"\x1b[C"),
        0x47 => KeyOut::seq(b"\x1b[H"),
        0x4F => KeyOut::seq(b"\x1b[F"),
        0x52 => KeyOut::seq(b"\x1b[2~"),
        0x53 => KeyOut::seq(b"\x1b[3~"),
        0x49 => KeyOut::seq(b"\x1b[5~"),
        0x51 => KeyOut::seq(b"\x1b[6~"),
        0x35 => KeyOut::seq(b"/"),
        0x1C => KeyOut::seq(b"\n"),
        _ => None,
    }
}

/// 事件流读取的结果（[`decode_into`] 的返回值）。
///
/// **为何要区分「读到 0 条」与「非记录字节」**（S09）：事件节点是**字符流**，
/// 理论上传入的字节缓冲恰为 16 的整数倍时不会出现半条；但若缓冲尺寸被调用方
/// 写错，**静默丢掉尾部半条**会让「按键丢失」与「调用方 bug」无法区分。
/// 故此处显式回报实际消耗字节数，由调用方自行核对。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordsRead {
    /// 已解析并喂入状态机的记录条数。
    pub count: usize,
    /// 本次实际消耗的字节数（恒为 `count * EVENT_RECORD_SIZE`）。
    pub bytes: usize,
}

/// 把一段**原始事件字节**逐条解析并喂入 `state`，产出的字节追加到 `out`。
///
/// **纯函数**（S23）：不碰 fd、不碰时钟，故可在宿主上完整测试。
/// 这是「事件流 → 字节流」的**唯一转换点**（S13），供 [`EventSourceReader`]
/// 与宿主测试共同使用，杜绝「测试测的与实际跑的是两套」（S06）。
///
/// 尾部不足一条记录的部分**不解析、不计入** `bytes`——它会被如实回报给调用方，
/// 由调用方决定是补齐缓冲区还是丢弃（本函数不猜测意图，S09）。
pub fn decode_into(state: &mut KeymapState, raw: &[u8], out: &mut alloc::vec::Vec<u8>) -> RecordsRead {
    let mut count = 0usize;
    let mut off = 0usize;
    while off + EVENT_RECORD_SIZE <= raw.len() {
        let chunk = &raw[off..off + EVENT_RECORD_SIZE];
        // 半条已由上面的循环条件排除，故 `parse_record` 必然成功；
        // 若失败说明内存布局被改动，属编程错误，如实跳过而非伪造（S09）。
        if let Ok(rec) = parse_record(chunk) {
            if let Some(k) = feed(state, &rec) {
                out.extend_from_slice(k.bytes());
            }
            count += 1;
        }
        off += EVENT_RECORD_SIZE;
    }
    RecordsRead { count, bytes: off }
}

/// `/devices/input/events` 的**阻塞**读取器（I-EVENTS 阶段 2 的消费端）。
///
/// # 职责边界
///
/// 本类型只管「把事件记录变成字节」；**编辑语义**（光标、历史、Tab 补全）
/// 仍归 `libline`。二者的粘合点是 `libline::InputSource`——见 `libline` 的
/// `EventSource`，它内部持有本类型。
///
/// # 为何读整条记录（S20 失效模式优先）
///
/// `read` 缓冲恒为 `16 * N` 字节，**绝不出现半条**。若用不足 16 字节的缓冲去读，
/// 事件节点会返回 `Ok(0)`（它的读侧契约是「不半条切割」），而那与「此刻无事件」
/// 无法区分——正是本类型要消除的歧义。
pub struct EventSourceReader {
    fd: u64,
    state: KeymapState,
    buf: [u8; EVENT_RECORD_SIZE * 8],
}

/// 事件流设备路径（单一事实源，S15）。
pub const EVENTS_PATH: &str = "/devices/input/events";

impl EventSourceReader {
    /// 打开事件节点；失败如实上抛（无该节点时**不**回退到键盘直读——
    /// 回退会让「事件流不可用」被静默掩盖，正是阶段 2 要避免的，S09）。
    pub fn open() -> Result<Self, Error> {
        let fd = crate::io::open(
            EVENTS_PATH,
            crate::io::OpenFlags::READ_ONLY,
            crate::io::Permissions::readonly(),
        )?;
        Ok(Self { fd, state: KeymapState::default(), buf: [0u8; EVENT_RECORD_SIZE * 8] })
    }

    /// 阻塞读取一批事件，把转换出的字节追加到 `out`，返回**新增字节数**。
    ///
    /// **空读不会返回 0**：内核在无事件时登记等待者并挂起本进程
    /// （`syscall.rs` 的 `input_event_stream()` 分支），IRQ1 到达后唤醒重试。
    /// 这正是 `libline::InputSource::refill` 契约要求的「阻塞到有数据」语义。
    ///
    /// 返回 0 只在**真的没有任何新字节**时发生（例如整批都是修饰键事件——
    /// 它们合法地不产字节）。调用方据此决定是否继续调用。
    pub fn read_into(&mut self, out: &mut alloc::vec::Vec<u8>) -> Result<usize, Error> {
        // **阻塞-唤醒哨兵：必须原样上抛，不得吞掉（关键，实测踩过）**。
        //
        // 内核在事件环空时**挂起本进程**，按键到达后以 `-EAGAIN` 哨兵唤醒，
        // 要求用户态**重试** `read`（`kernel/src/syscall.rs:2297`：
        // 「已挂起切走，用户态经哨兵重试 read」）。
        //
        // 【实测缺陷记录·两次都栽在这里】
        //
        // 初版：`crate::io::read(...)?` —— 把哨兵**当错误上抛**，
        //       `refill` 于是恒返回 `false`，按键完全无响应。
        //
        // 第二版（**错误方向**）：在这里把哨兵**吞成** `Ok(0)`，
        //       以为「本轮 0 字节」与「整批修饰键」同构。**但两者根本不同构**：
        //       修饰键是「数据已消费、下次再来」，哨兵是「**现在我就要你重试**」。
        //       吞掉后上层失去重试信号，只能靠外层循环空转——
        //       真机症状正是「会话冻结、宿主 CPU 28%」（§6.14.4i）。
        //
        // **正确形态（本版）**：哨兵**原样传播**给调用方，由它决定重试时机。
        // 对照证据：`evdemo`（同路径、同内核）在 `WouldBlock` 上直接
        // `continue` **立刻重试 read**，全程正常并在退出后把控制权交还 shell。
        // 两者唯一差别就是「是否保留了哨兵」，这就是本缺陷的判据。
        let n = crate::io::read(self.fd, &mut self.buf)?;

        let before = out.len();
        let _ = decode_into(&mut self.state, &self.buf[..n], out);
        Ok(out.len() - before)
    }

    /// 关闭底层 fd（显式，避免依赖 drop 的隐式副作用，S21）。
    pub fn close(self) -> Result<(), Error> {
        crate::io::close(self.fd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条键盘事件记录（kind: 1=down 2=up）。
    fn rec(kind: u8, e0: bool, code: u16) -> EventRecord {
        EventRecord { kind, flags: if e0 { EVENT_FLAG_E0 } else { 0 }, code, value: 0, timestamp: 0 }
    }
    fn down(e0: bool, code: u16) -> EventRecord { rec(EVENT_KIND_KEY_DOWN, e0, code) }
    fn up(e0: bool, code: u16) -> EventRecord { rec(EVENT_KIND_KEY_UP, e0, code) }
    fn bytes(s: &mut KeymapState, r: &EventRecord) -> alloc::vec::Vec<u8> {
        match feed(s, r) {
            Some(k) => k.bytes().to_vec(),
            None => alloc::vec::Vec::new(),
        }
    }

    #[test]
    fn test_parse_record_layout() {
        // ADR-047 §2.1 布局：kind/flags/code/value/timestamp 小端。
        let mut buf = [0u8; 16];
        buf[0] = 1; buf[1] = 1; buf[2] = 0x1D; buf[3] = 0;
        buf[8..16].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        let r = parse_record(&buf).unwrap();
        assert_eq!(r.kind, 1);
        assert_eq!(r.flags, EVENT_FLAG_E0);
        assert_eq!(r.code, 0x1D);
        assert_eq!(r.value, 0);
        assert_eq!(r.timestamp, 0x1122_3344_5566_7788);
        // 半条不是记录。
        assert!(parse_record(&buf[..15]).is_err());
    }

    #[test]
    fn test_basic_typing() {
        let mut s = KeymapState::default();
        // a 键按下 → 'a'；释放 → 空。
        assert_eq!(bytes(&mut s, &down(false, 0x1E)), b"a");
        assert_eq!(bytes(&mut s, &up(false, 0x1E)), b"");
        // 未知扫描码（0x00 表中无）→ 空。
        assert_eq!(bytes(&mut s, &down(false, 0x00)), b"");
    }

    #[test]
    fn test_shift_state_machine() {
        let mut s = KeymapState::default();
        // Shift 按下 → 状态置位（无字节）；再按 a → 'A'。
        assert_eq!(bytes(&mut s, &down(false, 0x2A)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x1E)), b"A");
        // Shift 释放 → 清位；再按 a → 'a'（KEY_UP 驱动状态机——事件化的表达力）。
        assert_eq!(bytes(&mut s, &up(false, 0x2A)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x1E)), b"a");
        // 右 Shift（0x36）同义。
        assert_eq!(bytes(&mut s, &down(false, 0x36)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x1E)), b"A");
        assert_eq!(bytes(&mut s, &up(false, 0x36)), b"");
    }

    #[test]
    fn test_shift_symbols() {
        let mut s = KeymapState::default();
        // Shift+7 → '&'（sendkey shift-7 真机同款）。
        assert_eq!(bytes(&mut s, &down(false, 0x2A)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x08)), b"&");
        // Shift+’.’ → '>'；Shift+'/' → '?'。
        assert_eq!(bytes(&mut s, &down(false, 0x33)), b"<");
        assert_eq!(bytes(&mut s, &down(false, 0x34)), b">");
        assert_eq!(bytes(&mut s, &down(false, 0x35)), b"?");
    }

    #[test]
    fn test_ctrl_folds_letters_only() {
        let mut s = KeymapState::default();
        // Ctrl 按下 → 状态；Ctrl+C → 0x03；Ctrl+D → 0x04。
        assert_eq!(bytes(&mut s, &down(false, 0x1D)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x2E)), b"\x03");
        assert_eq!(bytes(&mut s, &down(false, 0x20)), b"\x04");
        // Ctrl+数字/符号不折叠（S17 安全侧）：Ctrl+'1' → '1' 原样。
        assert_eq!(bytes(&mut s, &down(false, 0x02)), b"1");
        // Ctrl 释放后字母恢复。
        assert_eq!(bytes(&mut s, &up(false, 0x1D)), b"");
        assert_eq!(bytes(&mut s, &down(false, 0x2E)), b"c");
    }

    #[test]
    fn test_e0_sequences() {
        let mut s = KeymapState::default();
        // E0 方向键 → ANSI 序列；E0 Delete → \x1b[3~。
        assert_eq!(bytes(&mut s, &down(true, 0x48)), b"\x1b[A");
        assert_eq!(bytes(&mut s, &down(true, 0x50)), b"\x1b[B");
        assert_eq!(bytes(&mut s, &down(true, 0x4B)), b"\x1b[D");
        assert_eq!(bytes(&mut s, &down(true, 0x4D)), b"\x1b[C");
        assert_eq!(bytes(&mut s, &down(true, 0x53)), b"\x1b[3~");
        // 小键盘 '/'（E0 0x35）→ '/'；主键盘 '/'（0x35 无 E0）→ 查表 '/'（同字节）。
        assert_eq!(bytes(&mut s, &down(true, 0x35)), b"/");
        assert_eq!(bytes(&mut s, &down(false, 0x35)), b"/");
    }

    #[test]
    fn test_fkeys_and_numpad() {
        let mut s = KeymapState::default();
        // F1–F4 → SS3 序列。
        assert_eq!(bytes(&mut s, &down(false, 0x3B)), b"\x1bOP");
        assert_eq!(bytes(&mut s, &down(false, 0x3C)), b"\x1bOQ");
        // F5 → \x1b[15~（最长序列，KeyOut 8 字节数组的边界证据）。
        assert_eq!(bytes(&mut s, &down(false, 0x3F)), b"\x1b[15~");
        // NumLock 小键盘（无 E0 的 0x47 区域）→ 数字。
        assert_eq!(bytes(&mut s, &down(false, 0x47)), b"7");
        assert_eq!(bytes(&mut s, &down(false, 0x53)), b".");
        // 注意：主键盘 Esc 也是 0x01（查表得 27），优先查表命中。
        assert_eq!(bytes(&mut s, &down(false, 0x01)), b"\x1b");
    }

    #[test]
    fn test_unknown_kind_ignored() {
        let mut s = KeymapState::default();
        // 指针预留 kind（3/4/5）：本模块不认识，不产出。
        assert_eq!(bytes(&mut s, &rec(3, false, 0x1E)), b"");
        assert_eq!(bytes(&mut s, &rec(4, false, 0x1E)), b"");
    }

    #[test]
    fn test_constants_mirror_kernel() {
        // 布局常量与内核镜像（PRE-12 同款纪律：ABI 常量两侧逐位一致）。
        assert_eq!(EVENT_RECORD_SIZE, 16);
        assert_eq!(EVENT_KIND_KEY_DOWN, 1);
        assert_eq!(EVENT_KIND_KEY_UP, 2);
        assert_eq!(EVENT_FLAG_E0, 1);
        assert_eq!(EVENT_FLAG_NO_TIME, 2);
    }

    // ---------------- 阶段 2：记录批量解码（`decode_into`） ----------------

    /// 把若干事件序列化成事件流的**线格式**（16 字节/条，小端）。
    fn wire(recs: &[EventRecord]) -> alloc::vec::Vec<u8> {
        let mut v = alloc::vec::Vec::new();
        for r in recs {
            let lo = (r.kind as u64)
                | ((r.flags as u64) << 8)
                | ((r.code as u64) << 16)
                | ((r.value as u64) << 32);
            v.extend_from_slice(&lo.to_le_bytes());
            v.extend_from_slice(&r.timestamp.to_le_bytes());
        }
        v
    }

    /// **等价性（本小点的核心判据）**：逐条 `feed` 与成批 `decode_into`
    /// 必须产出**完全相同**的字节——否则「批量路径」就是第二套语义（S13）。
    #[test]
    fn test_decode_into_matches_per_record_feed() {
        let seq = [
            down(false, 0x2A),   // Shift 下
            down(false, 0x1E),   // A
            up(false, 0x1E),
            up(false, 0x2A),     // Shift 上
            down(false, 0x2E),   // c
            down(false, 0x1D),   // Ctrl 下
            down(false, 0x2E),   // ^C（Ctrl 折叠字母）
            up(false, 0x2E),
            up(false, 0x1D),
            down(true, 0x48),    // E0 上箭头
            down(false, 0x1C),   // 回车
        ];
        // 逐条路径。
        let mut s1 = KeymapState::default();
        let mut expect = alloc::vec::Vec::new();
        for r in &seq {
            expect.extend_from_slice(&bytes(&mut s1, r));
        }
        // 批量路径（一次喂入全部记录的线格式）。
        let mut s2 = KeymapState::default();
        let mut got = alloc::vec::Vec::new();
        let rr = decode_into(&mut s2, &wire(&seq), &mut got);
        assert_eq!(rr.count, seq.len(), "must decode every record");
        assert_eq!(rr.bytes, seq.len() * EVENT_RECORD_SIZE);
        assert_eq!(got, expect, "batch path must equal per-record path");
        // 非空的可读性检查：本序列确实产出了字节（避免"两边都空"的假绿）。
        assert!(expect.contains(&0x03), "sequence must yield ^C (0x03): {:?}", expect);
        assert!(expect.contains(&b'A'), "sequence must yield Shift-A");
    }

    /// **分批到达也算数**：事件流是字符设备，一次 `read` 可能只回来半批。
    /// 分两次喂入与一次喂入必须等价——状态机跨调用保持（这正是
    /// `EventSourceReader` 持有 `KeymapState` 的理由）。
    #[test]
    fn test_decode_into_state_survives_split_batches() {
        let seq = [down(false, 0x2A), down(false, 0x1E)]; // Shift 下, A
        let w = wire(&seq);
        let mut s = KeymapState::default();
        let mut out = alloc::vec::Vec::new();
        // 第一批只给第一条记录（Shift 按下）——产 0 字节但**必须留下状态**。
        let rr1 = decode_into(&mut s, &w[..EVENT_RECORD_SIZE], &mut out);
        assert_eq!(rr1.count, 1);
        assert!(out.is_empty(), "Shift alone yields no bytes");
        // 第二批给第二条记录——上面的 Shift 状态必须还在，故产出大写 A。
        let rr2 = decode_into(&mut s, &w[EVENT_RECORD_SIZE..], &mut out);
        assert_eq!(rr2.count, 1);
        assert_eq!(out, b"A", "Shift state must survive across batches");
    }

    /// **半条不解析、不伪造**（S09）：尾部不足 16 字节时
    /// `count`/`bytes` 只计完整记录，半条**不被**当作记录消费。
    #[test]
    fn test_decode_into_ignores_partial_trailing_record() {
        let mut raw = wire(&[down(false, 0x1E)]); // 完整一条（"a"）
        raw.extend_from_slice(&[0u8; 7]);        // 尾巴 7 字节 = 半条
        let mut s = KeymapState::default();
        let mut out = alloc::vec::Vec::new();
        let rr = decode_into(&mut s, &raw, &mut out);
        assert_eq!(rr.count, 1, "only the complete record counts");
        assert_eq!(rr.bytes, EVENT_RECORD_SIZE, "partial tail not consumed");
        assert_eq!(out, b"a");
    }

    /// 空输入是合法的「0 条」，不是错误、也不产字节。
    #[test]
    fn test_decode_into_empty_input() {
        let mut s = KeymapState::default();
        let mut out = alloc::vec::Vec::new();
        let rr = decode_into(&mut s, &[], &mut out);
        assert_eq!(rr.count, 0);
        assert_eq!(rr.bytes, 0);
        assert!(out.is_empty());
    }

    /// 指针类 kind（3/4/5，ADR-047 预留）本模块不认识：
    /// **不计入 count、不产字节**，且**不得**让整批解码中断。
    #[test]
    fn test_decode_into_unknown_kind_does_not_abort_batch() {
        let unknown = EventRecord { kind: 3, flags: 0, code: 0, value: 0, timestamp: 0 };
        let seq = [unknown, down(false, 0x1E)];
        let mut s = KeymapState::default();
        let mut out = alloc::vec::Vec::new();
        let rr = decode_into(&mut s, &wire(&seq), &mut out);
        // 两条都被解析（都在 16 字节边界上），第 2 条照常产出。
        assert_eq!(rr.count, 2, "unknown kind is still a parsed record");
        assert_eq!(out, b"a", "batch must continue past an unknown kind");
    }
}

