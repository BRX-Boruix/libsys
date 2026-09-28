# libsys

BORUIX 的**用户态系统调用库**：把内核提供的系统调用封装成 Rust 函数，供所有用户态程序使用。

[English](README.en.md)

## 它做什么

用户态程序不能直接执行系统调用——必须按约定的方式传参、触发、检查结果。本库封装了这套约定：

```rust
use libsys::{open, read, write, close};
```

## 覆盖范围

共封装 **101 个系统调用**，按领域分模块：

| 领域 | 内容 |
| --- | --- |
| **文件与 I/O** | 打开、读写、关闭、目录操作、管道 |
| **内存** | 堆扩展、内存映射与解除映射 |
| **进程** | 派生、等待、退出、身份查询与切换、进程组 |
| **线程** | 派生、汇合、退出、让出 |
| **同步** | 创建、删除、等待、唤醒 |
| **信号** | 发送、注册处理、屏蔽 |
| **时间** | 单调时钟、墙钟、睡眠 |
| **设备与卷** | 设备事件、卷枚举与挂载卸载 |
| **驱动** | 设备登记、认领、映射、查询 |
| **音频** | 消费者附加、取帧、提交 |
| **其他** | 随机数、JSON 解析、系统信息、关机与重启 |

## 使用

作为依赖被其他用户态程序引用，通常不单独运行：

```toml
[dependencies]
libsys = { path = "../libsys" }
```

本库注册了用户态的**堆分配器**，因此可以直接使用 `Vec`、`String`、`Box` 等标准容器。堆在首次
分配时向内核申请，之后按需增长。

## 构建

```bash
cargo build --release
```

## 文件结构

```
libsys/src/
├── lib.rs        # 模块导出与重导出
├── nr.rs         # 系统调用号定义
├── syscall.rs    # 系统调用触发与错误转换
├── error.rs      # 错误类型
├── start.rs      # 程序入口点
├── allocator.rs  # 堆分配器
├── io.rs         # 文件与 I/O
├── mem.rs        # 内存
├── process.rs    # 进程
├── thread.rs     # 线程
├── sync.rs       # 同步
├── signal.rs     # 信号
├── time.rs       # 时间
├── volume.rs     # 设备与卷
├── driver.rs     # 驱动
├── audio.rs      # 音频
├── event.rs      # 事件流与键位映射
├── pipe.rs       # 管道
├── power.rs      # 关机与重启
├── random.rs     # 随机数
├── json.rs       # JSON 解析
├── object.rs     # 对象动词（流、同步、任务、文件系统）
├── system.rs     # 系统信息
└── builtins.rs   # 内建辅助
```

## 相关项目

- [`libc`](https://github.com/BRX-Boruix/libc) —— C 标准库接口，构建于本库之上
- [`libline`](https://github.com/BRX-Boruix/libline) —— 行编辑库，使用本库的输入接口
- [`selftest`](https://github.com/BRX-Boruix/selftest) —— 端到端验收的宿主程序

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。
