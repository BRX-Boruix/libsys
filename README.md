# libsys

BORUIX 的用户态系统调用库。用户态程序不能直接执行系统调用，必须按约定的方式传参、触发、检查
结果——本库把这套约定封装成 Rust 函数。

[English](README.en.md)

## 覆盖范围

共封装 **101 个系统调用**，分为以下领域：

- 文件与 I/O：打开、读写、关闭、目录操作、管道
- 内存：堆扩展、映射与解除映射
- 进程与线程：派生、等待、退出、身份查询与切换、汇合、让出
- 同步与信号：创建、删除、等待、唤醒、发送、注册处理、屏蔽
- 时间：单调时钟、墙钟、睡眠
- 设备、卷与驱动：设备事件、卷枚举与挂载卸载、设备登记与认领
- 音频：消费者附加、取帧、提交
- 其他：随机数、JSON 解析、系统信息、关机与重启

## 使用

作为依赖被其他用户态程序引用，通常不单独运行：

```toml
[dependencies]
libsys = { path = "../libsys" }
```

本库注册了用户态的堆分配器，因此可以直接使用 `Vec`、`String`、`Box` 等标准容器。堆在首次分配时
向内核申请，之后按需增长。

## 构建

```bash
cargo build --release
```

## 仓库布局

- `src/lib.rs` —— 模块导出与重导出
- `src/nr.rs` —— 系统调用号定义
- `src/syscall.rs` —— 系统调用触发与错误转换
- `src/start.rs` —— 程序入口点
- `src/allocator.rs` —— 堆分配器
- `src/io.rs`、`mem.rs`、`process.rs`、`thread.rs`、`sync.rs`、`signal.rs`、`time.rs` —— 各领域接口
- `src/volume.rs`、`driver.rs`、`audio.rs`、`event.rs` —— 设备、驱动与音频
- `src/pipe.rs`、`power.rs`、`random.rs`、`json.rs`、`object.rs`、`system.rs` —— 其余接口

## 相关项目

- [`libc`](https://github.com/BRX-Boruix/libc) —— C 标准库接口，构建于本库之上
- [`libline`](https://github.com/BRX-Boruix/libline) —— 行编辑库，使用本库的输入接口
- [`selftest`](https://github.com/BRX-Boruix/selftest) —— 端到端验收的宿主程序

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。
