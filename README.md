# BORUIX libsys

系统调用封装层：用户态程序与内核之间的 ABI 边界。

## 职责
- 定义系统调用号（syscall number）的约定
- 为每个系统调用提供薄封装（`syscall()` 入口 + 类型安全的包装函数）
- 作为宏内核"可替换内核"的唯一稳定契约

## 依赖关系
```
libsys  ←  (无依赖)
  ▲
libc   ←  libsys
```
内核在 `kernel` 侧实现与 `libsys` 约定一致的 syscall ABI。

## 内容规划
- `src/syscall.rs`   — 系统调用入口与参数约定
- `src/nr.rs`        — 系统调用号定义
- `src/abi.rs`       — 与内核共享的 ABI 类型
