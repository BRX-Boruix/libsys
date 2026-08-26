//! 轻量流式 JSON 构造器与解析辅助（支持栈/堆双模）。
//!
//! 面向 `no_std` 用户态（ADR-013 JSON 第一公民）：
//! - 支持零堆分配（基于固定栈缓冲区）与动态堆 `Vec`/`String` 构造；
//! - 自动字符串转义处理；
//! - 键值对、嵌套对象、数组列表流式输出。

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// JSON 写入目标 Trait。
pub trait JsonTarget {
    fn write_str(&mut self, s: &str) -> Result<(), ()>;
    fn write_char(&mut self, c: char) -> Result<(), ()> {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        self.write_str(encoded)
    }
}

/// 栈缓冲区写入目标（固定容量，零堆分配）。
pub struct StackTarget<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> StackTarget<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; N],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> JsonTarget for StackTarget<N> {
    fn write_str(&mut self, s: &str) -> Result<(), ()> {
        let bytes = s.as_bytes();
        if self.len + bytes.len() > N {
            return Err(());
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

/// 堆动态 `Vec<u8>` 写入目标。
pub struct VecTarget {
    vec: Vec<u8>,
}

impl VecTarget {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            vec: Vec::with_capacity(cap),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.vec
    }

    pub fn into_string(self) -> Result<String, alloc::string::FromUtf8Error> {
        String::from_utf8(self.vec)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.vec
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.vec).unwrap_or("")
    }
}

impl JsonTarget for VecTarget {
    fn write_str(&mut self, s: &str) -> Result<(), ()> {
        self.vec.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

/// 堆动态 `String` 写入目标。
impl JsonTarget for String {
    fn write_str(&mut self, s: &str) -> Result<(), ()> {
        self.push_str(s);
        Ok(())
    }
}

/// 支持可变引用作为写入目标。
impl<T: JsonTarget + ?Sized> JsonTarget for &mut T {
    fn write_str(&mut self, s: &str) -> Result<(), ()> {
        (**self).write_str(s)
    }

    fn write_char(&mut self, c: char) -> Result<(), ()> {
        (**self).write_char(c)
    }
}

/// 写入转义后的 JSON 字符串内容（带两端双引号）。
pub fn write_escaped_str<T: JsonTarget + ?Sized>(target: &mut T, s: &str) -> Result<(), ()> {
    target.write_char('"')?;
    for c in s.chars() {
        match c {
            '"' => target.write_str("\\\"")?,
            '\\' => target.write_str("\\\\")?,
            '\n' => target.write_str("\\n")?,
            '\r' => target.write_str("\\r")?,
            '\t' => target.write_str("\\t")?,
            '\x08' => target.write_str("\\b")?,
            '\x0C' => target.write_str("\\f")?,
            c if (c as u32) < 0x20 => {
                let val = c as u32;
                target.write_str("\\u00")?;
                let h1 = (val >> 4) & 0xf;
                let h0 = val & 0xf;
                let hex_char = |n: u32| -> char {
                    if n < 10 {
                        (b'0' + n as u8) as char
                    } else {
                        (b'a' + (n - 10) as u8) as char
                    }
                };
                target.write_char(hex_char(h1))?;
                target.write_char(hex_char(h0))?;
            }
            c => target.write_char(c)?,
        }
    }
    target.write_char('"')?;
    Ok(())
}

/// 流式 JSON 写入器。
pub struct JsonWriter<T: JsonTarget> {
    target: T,
}

impl<T: JsonTarget> JsonWriter<T> {
    pub fn new(target: T) -> Self {
        Self { target }
    }

    pub fn into_target(self) -> T {
        self.target
    }

    pub fn target_ref(&self) -> &T {
        &self.target
    }

    pub fn target_mut(&mut self) -> &mut T {
        &mut self.target
    }

    /// 开始写入根对象 `{`
    pub fn start_object(&mut self) -> Result<JsonObject<'_, T>, ()> {
        self.target.write_char('{')?;
        Ok(JsonObject {
            target: &mut self.target,
            first: true,
        })
    }

    /// 开始写入根数组 `[`
    pub fn start_array(&mut self) -> Result<JsonArray<'_, T>, ()> {
        self.target.write_char('[')?;
        Ok(JsonArray {
            target: &mut self.target,
            first: true,
        })
    }
}

/// JSON 对象构建器。
pub struct JsonObject<'a, T: JsonTarget> {
    target: &'a mut T,
    first: bool,
}

impl<'a, T: JsonTarget> JsonObject<'a, T> {
    fn write_key(&mut self, key: &str) -> Result<(), ()> {
        if !self.first {
            self.target.write_char(',')?;
        }
        self.first = false;
        write_escaped_str(self.target, key)?;
        self.target.write_char(':')?;
        Ok(())
    }

    /// 写入字符串属性 `"key":"value"`
    pub fn field_str(&mut self, key: &str, val: &str) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        write_escaped_str(self.target, val)?;
        Ok(self)
    }

    /// 写入整型属性 `"key":123`
    pub fn field_i64(&mut self, key: &str, val: i64) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        let mut buf = [0u8; 32];
        let s = i64_to_str(val, &mut buf);
        self.target.write_str(s)?;
        Ok(self)
    }

    /// 写入无符号整型属性 `"key":123`
    pub fn field_u64(&mut self, key: &str, val: u64) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        let mut buf = [0u8; 32];
        let s = u64_to_str(val, &mut buf);
        self.target.write_str(s)?;
        Ok(self)
    }

    /// 写入布尔属性 `"key":true`
    pub fn field_bool(&mut self, key: &str, val: bool) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        self.target.write_str(if val { "true" } else { "false" })?;
        Ok(self)
    }

    /// 写入 null 属性 `"key":null`
    pub fn field_null(&mut self, key: &str) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        self.target.write_str("null")?;
        Ok(self)
    }

    /// 写入原始未经转义的 JSON 片段 `"key":{...}` 或 `"key":[...]`
    pub fn field_raw(&mut self, key: &str, raw_json: &str) -> Result<&mut Self, ()> {
        self.write_key(key)?;
        self.target.write_str(raw_json)?;
        Ok(self)
    }

    /// 开启嵌套子对象 `"key":{`
    pub fn sub_object<F>(&mut self, key: &str, f: F) -> Result<&mut Self, ()>
    where
        F: FnOnce(&mut JsonObject<'_, T>) -> Result<(), ()>,
    {
        self.write_key(key)?;
        self.target.write_char('{')?;
        let mut sub = JsonObject {
            target: self.target,
            first: true,
        };
        f(&mut sub)?;
        sub.end()?;
        Ok(self)
    }

    /// 开启嵌套子数组 `"key":[`
    pub fn sub_array<F>(&mut self, key: &str, f: F) -> Result<&mut Self, ()>
    where
        F: FnOnce(&mut JsonArray<'_, T>) -> Result<(), ()>,
    {
        self.write_key(key)?;
        self.target.write_char('[')?;
        let mut sub = JsonArray {
            target: self.target,
            first: true,
        };
        f(&mut sub)?;
        sub.end()?;
        Ok(self)
    }

    /// 结束并闭合对象 `}`
    pub fn end(self) -> Result<(), ()> {
        self.target.write_char('}')
    }
}

/// JSON 数组构建器。
pub struct JsonArray<'a, T: JsonTarget> {
    target: &'a mut T,
    first: bool,
}

impl<'a, T: JsonTarget> JsonArray<'a, T> {
    fn write_comma(&mut self) -> Result<(), ()> {
        if !self.first {
            self.target.write_char(',')?;
        }
        self.first = false;
        Ok(())
    }

    /// 写入字符串元素
    pub fn push_str(&mut self, val: &str) -> Result<&mut Self, ()> {
        self.write_comma()?;
        write_escaped_str(self.target, val)?;
        Ok(self)
    }

    /// 写入无符号整型元素
    pub fn push_u64(&mut self, val: u64) -> Result<&mut Self, ()> {
        self.write_comma()?;
        let mut buf = [0u8; 32];
        let s = u64_to_str(val, &mut buf);
        self.target.write_str(s)?;
        Ok(self)
    }

    /// 写入有符号整型元素
    pub fn push_i64(&mut self, val: i64) -> Result<&mut Self, ()> {
        self.write_comma()?;
        let mut buf = [0u8; 32];
        let s = i64_to_str(val, &mut buf);
        self.target.write_str(s)?;
        Ok(self)
    }

    /// 写入布尔元素
    pub fn push_bool(&mut self, val: bool) -> Result<&mut Self, ()> {
        self.write_comma()?;
        self.target.write_str(if val { "true" } else { "false" })?;
        Ok(self)
    }

    /// 写入 null 元素
    pub fn push_null(&mut self) -> Result<&mut Self, ()> {
        self.write_comma()?;
        self.target.write_str("null")?;
        Ok(self)
    }

    /// 写入 raw JSON 元素
    pub fn push_raw(&mut self, raw_json: &str) -> Result<&mut Self, ()> {
        self.write_comma()?;
        self.target.write_str(raw_json)?;
        Ok(self)
    }

    /// 开启子对象元素 `{`
    pub fn push_object<F>(&mut self, f: F) -> Result<&mut Self, ()>
    where
        F: FnOnce(&mut JsonObject<'_, T>) -> Result<(), ()>,
    {
        self.write_comma()?;
        self.target.write_char('{')?;
        let mut sub = JsonObject {
            target: self.target,
            first: true,
        };
        f(&mut sub)?;
        sub.end()?;
        Ok(self)
    }

    /// 开启子数组元素 `[`
    pub fn push_array<F>(&mut self, f: F) -> Result<&mut Self, ()>
    where
        F: FnOnce(&mut JsonArray<'_, T>) -> Result<(), ()>,
    {
        self.write_comma()?;
        self.target.write_char('[')?;
        let mut sub = JsonArray {
            target: self.target,
            first: true,
        };
        f(&mut sub)?;
        sub.end()?;
        Ok(self)
    }

    /// 结束并闭合数组 `]`
    pub fn end(self) -> Result<(), ()> {
        self.target.write_char(']')
    }
}

/// u64 转字符串（零分配）。
fn u64_to_str(mut val: u64, buf: &mut [u8; 32]) -> &str {
    if val == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut pos = 32;
    while val > 0 {
        pos -= 1;
        buf[pos] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    core::str::from_utf8(&buf[pos..32]).unwrap_or("")
}

/// i64 转字符串（零分配）。
fn i64_to_str(val: i64, buf: &mut [u8; 32]) -> &str {
    if val >= 0 {
        return u64_to_str(val as u64, buf);
    }
    let mut uval = val.unsigned_abs();
    let mut pos = 32;
    while uval > 0 {
        pos -= 1;
        buf[pos] = b'0' + (uval % 10) as u8;
        uval /= 10;
    }
    pos -= 1;
    buf[pos] = b'-';
    core::str::from_utf8(&buf[pos..32]).unwrap_or("")
}

// ===========================================================================
// JSON Parser（ADR-024：用户态解析，内核不碰）
//
// 2026-09-17 从 `shell/src/tree_json.rs` 迁入，供 init / shell / 未来的
// 用户态守护进程共享。本模块只有 encoder 时是"输出结构化数据"的安全操作；
// parser 的输入是不可信字节流，必须处理各种畸形情况——这是它不进入内核
// （klib）的根本原因。语法树渲染（树状图打印）属 shell 呈现层，留在 shell
// 侧 `shell/src/json_tree.rs`，本模块只负责解析。
// ===========================================================================

/// JSON 解析结果值（保留原始词法形态：数值以字符串保存，不做精度转换）。
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

/// JSON 解析器（基于字符流迭代器，递归下降）。
pub struct JsonParser<'a> {
    chars: core::str::Chars<'a>,
    peeked: Option<char>,
}

impl<'a> JsonParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars(),
            peeked: None,
        }
    }

    fn peek(&mut self) -> Option<char> {
        if self.peeked.is_none() {
            self.peeked = self.chars.next();
        }
        self.peeked
    }

    fn next(&mut self) -> Option<char> {
        if let Some(c) = self.peeked.take() {
            Some(c)
        } else {
            self.chars.next()
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.next();
            } else {
                break;
            }
        }
    }

    /// 解析一个完整 JSON 值。失败返回描述性 `&'static str`（不 panic）。
    pub fn parse(&mut self) -> Result<JsonValue, &'static str> {
        self.skip_whitespace();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            _ => Err("unexpected character in json"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, &'static str> {
        self.next(); // '{'
        let mut fields = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('}') {
                self.next();
                break;
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.next() != Some(':') {
                return Err("expected ':' after key");
            }
            let val = self.parse()?;
            fields.push((key, val));

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.next();
                }
                Some('}') => {
                    self.next();
                    break;
                }
                _ => return Err("expected ',' or '}' in object"),
            }
        }
        Ok(JsonValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<JsonValue, &'static str> {
        self.next(); // '['
        let mut items = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some(']') {
                self.next();
                break;
            }
            let val = self.parse()?;
            items.push(val);

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.next();
                }
                Some(']') => {
                    self.next();
                    break;
                }
                _ => return Err("expected ',' or ']' in array"),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, &'static str> {
        self.skip_whitespace();
        if self.next() != Some('"') {
            return Err("expected '\"'");
        }
        let mut s = String::new();
        while let Some(c) = self.next() {
            match c {
                '"' => return Ok(s),
                '\\' => match self.next() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('/') => s.push('/'),
                    Some('b') => s.push('\x08'),
                    Some('f') => s.push('\x0C'),
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('u') => {
                        // 简化 4 位十六进制解析：非法转义不 panic，截断/越界
                        // 统一落为 '?' 占位，保持"宁可怪异不可崩溃"的用户态边界。
                        let mut hex_val = 0u32;
                        for _ in 0..4 {
                            if let Some(hc) = self.next() {
                                if let Some(d) = hc.to_digit(16) {
                                    hex_val = (hex_val << 4) | d;
                                }
                            }
                        }
                        if let Some(ch) = char::from_u32(hex_val) {
                            s.push(ch);
                        } else {
                            s.push('?');
                        }
                    }
                    _ => s.push('?'),
                },
                other => s.push(other),
            }
        }
        Err("unclosed string")
    }

    fn parse_number(&mut self) -> Result<JsonValue, &'static str> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' || c.is_ascii_digit() {
                s.push(self.next().unwrap());
            } else {
                break;
            }
        }
        Ok(JsonValue::Number(s))
    }

    fn parse_bool(&mut self) -> Result<JsonValue, &'static str> {
        if self.peek() == Some('t') {
            for expected in "true".chars() {
                if self.next() != Some(expected) {
                    return Err("expected true");
                }
            }
            Ok(JsonValue::Bool(true))
        } else {
            for expected in "false".chars() {
                if self.next() != Some(expected) {
                    return Err("expected false");
                }
            }
            Ok(JsonValue::Bool(false))
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, &'static str> {
        for expected in "null".chars() {
            if self.next() != Some(expected) {
                return Err("expected null");
            }
        }
        Ok(JsonValue::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_stack_json_object() {
        let mut target = StackTarget::<256>::new();
        {
            let mut writer = JsonWriter::new(&mut target);
            let mut obj = writer.start_object().unwrap();
            obj.field_str("name", "borui-user").unwrap();
            obj.field_u64("version", 1).unwrap();
            obj.field_bool("active", true).unwrap();
            obj.field_null("parent").unwrap();
            obj.end().unwrap();
        }
        assert_eq!(
            target.as_str(),
            r#"{"name":"borui-user","version":1,"active":true,"parent":null}"#
        );
    }

    // ---- 以下为自 shell/src/tree_json.rs 随迁的 parser 测试（ADR-024） ----

    #[test]
    fn test_parse_nested_object() {
        let json_str = r#"{"arch":"x86_64","cores":4,"features":["smap","smep"],"status":{"online":true,"uptime":12345}}"#;
        let mut parser = JsonParser::new(json_str);
        let val = parser.parse().expect("parse failed");

        match &val {
            JsonValue::Object(fields) => {
                assert_eq!(fields.len(), 4);
                assert_eq!(fields[0].0, "arch");
                assert_eq!(fields[0].1, JsonValue::String("x86_64".into()));
                assert_eq!(fields[1].0, "cores");
                assert_eq!(fields[1].1, JsonValue::Number("4".into()));
                assert_eq!(
                    fields[2].1,
                    JsonValue::Array(vec![
                        JsonValue::String("smap".into()),
                        JsonValue::String("smep".into()),
                    ])
                );
                assert_eq!(
                    fields[3].1,
                    JsonValue::Object(vec![
                        ("online".into(), JsonValue::Bool(true)),
                        ("uptime".into(), JsonValue::Number("12345".into())),
                    ])
                );
            }
            _ => panic!("root must be object"),
        }
    }

    #[test]
    fn test_parse_primitive_values() {
        let cases: &[(&str, JsonValue)] = &[
            ("null", JsonValue::Null),
            ("true", JsonValue::Bool(true)),
            ("false", JsonValue::Bool(false)),
            ("-12.5e3", JsonValue::Number("-12.5e3".into())),
            (r#""hello""#, JsonValue::String("hello".into())),
            ("[]", JsonValue::Array(vec![])),
            ("{}", JsonValue::Object(vec![])),
        ];
        for (input, expected) in cases {
            let mut parser = JsonParser::new(input);
            assert_eq!(parser.parse().unwrap(), *expected, "input: {input}");
        }
    }

    #[test]
    fn test_parse_string_escapes() {
        let mut parser = JsonParser::new(r#""a\"b\\c\/d\n\t""#);
        assert_eq!(
            parser.parse().unwrap(),
            JsonValue::String("a\"b\\c/d\n\t".into())
        );
    }

    #[test]
    fn test_parse_whitespace_tolerance() {
        let mut parser = JsonParser::new("  \n\t { \"a\" : [ 1 , 2 ] } \r\n ");
        assert_eq!(
            parser.parse().unwrap(),
            JsonValue::Object(vec![(
                "a".into(),
                JsonValue::Array(vec![JsonValue::Number("1".into()), JsonValue::Number("2".into())])
            )])
        );
    }

    #[test]
    fn test_parse_rejects_malformed() {
        // 对抗输入：畸形 JSON 必须报错，绝不 panic。
        let bad_cases: &[&str] = &[
            "",              // 空输入
            "{",             // 未闭合对象
            "[1,",           // 尾逗号 + 未闭合数组
            r#"{"a" 1}"#,    // 缺冒号
            r#"{"a":"b" "c"}"#, // 缺逗号
            "tru",           // 截断布尔
            "\"unclosed",    // 未闭合字符串
            "boom",          // 纯垃圾
            "nul",           // 截断 null
        ];
        for input in bad_cases {
            let mut parser = JsonParser::new(input);
            assert!(parser.parse().is_err(), "input {input:?} must fail");
        }
    }
}
