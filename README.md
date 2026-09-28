# libsys

**简体中文** | [English](#english)

BORUIX 的**系统调用封装层**——用户态程序与内核之间的那道边界。

它定义了双方约定的调用接口，把底层的裸调用包装成 Rust 风格的安全接口，并提供用户程序的入口。

```
用户程序  →  libsys（本库）  →  内核
```

---

## 它解决什么问题

用户态程序想读文件、申请内存、创建线程，都必须请求内核代劳。这个"请求"的机制是**约定**：程序把
调用号和参数放进指定的寄存器，执行一条特殊指令，内核据此分发。

裸地用这套机制是可行的，但很痛苦——要处理寄存器约定、要检查返回值的错误标记、要记得每个系统
调用号的数字。`libsys` 把这些收进一层薄封装：调用者写 `read(fd, &mut buf)`，拿到
`Result`，不必关心底下的寄存器长什么样。

**为什么强调"薄"**：好的封装层不应该隐藏或改变语义。系统调用的错误码、阻塞行为、参数含义都应该
如实透传。一旦封装层开始"帮你处理"某些情况，它就成了新的语义来源，而真相在内核里——两者分叉
时极难排查。

## 用户程序怎么写

`libsys` 提供程序入口。用户程序只需导出一个 `user_main`：

```rust
#![no_std]
#![no_main]

use libsys::{write, STDOUT};

#[unsafe(no_mangle)]
pub extern "C" fn user_main(_argc: isize, _argv: *const *const u8) -> i32 {
    let _ = write(STDOUT, b"hello from userspace\n");
    0
}
```

入口负责从栈上取出参数、对齐栈、调用 `user_main`，并把它的返回值作为进程退出码。
`user_main` 是用户程序**唯一需要导出的符号**。

## 系统调用是怎么编号的

系统调用号不是随手递增的，而是一套**正交的命名方案**：调用号由两部分拼成——

```
(资源域 << 4) | 动词
```

**资源域**表示操作哪一类东西，**动词**表示做什么：

| 资源域 | 含义 | 动词 | 含义 |
| --- | --- | --- | --- |
| `STREAM` | 字节流 | `CREATE` | 创建 |
| `MEMORY` | 内存 | `READ` | 读 |
| `TASK` | 进程与线程 | `WRITE` | 写 |
| `VFS` | 文件系统 | `DELETE` | 删除 |
| `DEVICE` | 设备 | | |

这样设计的好处是**可读性直接编码进数字**：看到调用号的高位就知道它操作什么，低位就知道它做什么，
不必去翻对照表。额外的资源域（同步、信号、电源、音频、卷管理）沿用同一方案扩展。

## 怎么发起调用

通过一条特殊指令陷入内核。约定是：

| 寄存器 | 用途 |
| --- | --- |
| `rax` | 系统调用号 |
| `rdi` / `rsi` / `rdx` / `r10` / `r8` / `r9` | 六个参数 |

后三个参数之所以放在不常用的寄存器里，是因为它们不能被编译器生成的普通代码占用——这一点在
汇编层显式钉住，否则内核会从保存的现场读到垃圾值。

## 错误是怎么表示的

调用返回后，**最高位是错误标记**：置位表示出错，其余位是取负的错误码。封装层把它解包成 Rust 的
`Result`，调用者于是不必手工检查标志位。

错误码与内核**同源**——同一个含义在两边的数值必须一致，否则调用者按错误的语义处理。

## 模块

这个库覆盖了系统调用接口的各个领域：

| 领域 | 内容 |
| --- | --- |
| 调用机制 | 调用号定义、陷入指令、错误解包 |
| 输入输出 | 文件打开读写、目录遍历、文件属性、权限 |
| 进程 | 创建、等待、退出、信号、工作查询、身份与组 |
| 线程 | 创建、汇合、退出、同步原语 |
| 内存 | 堆扩展、内存映射 |
| 设备与驱动 | 设备认领、DMA 分配、中断等待、拓扑事件 |
| 卷管理 | 挂载、卸载、列举、探测 |
| 音频 | 音频流控制 |
| 其他 | 时间、随机数、电源管理、JSON 解析、事件转换 |

## 事件转换层

库里包含一个**把输入事件转换成终端字节**的模块：处理键码映射、修饰键状态、控制键折叠。

它住在用户态而不是内核里，是一个有意的分工——**键码布局是策略，不是机制**。内核只负责把"按下
了哪个键"原样交给用户态，布局与转义规则留在用户态，就可以在不改内核的前提下调整。内核的输入
因此保持为原始事件流。

## 服务对象

`libsys` **不依赖任何其他 BORUIX 组件**，是整个用户态的最底层。它同时服务两类使用者：

- **Rust 程序**——直接依赖调用
- **C 程序**——经由 [`libc`](https://github.com/BRX-Boruix/libc)（用 Rust 实现、暴露 C 接口）
  间接使用同一套系统调用

## 测试

库内含 **60 个单元测试**，覆盖可脱离设备验证的部分——错误解包、参数处理、事件转换、JSON 解析等。

## 构建

```bash
cargo build --release
cargo test
```

## 文件结构

```
libsys/
└── src/
    ├── lib.rs        # 公开接口
    ├── nr.rs         # 系统调用号定义
    ├── syscall.rs    # 陷入入口与错误解包
    ├── error.rs      # 错误码
    ├── start.rs      # 用户程序入口
    └── ...           # 各领域封装
```

## 相关项目

- [`libc`](https://github.com/BRX-Boruix/libc) —— C 标准库，构建于本库之上
- [`csrc`](https://github.com/BRX-Boruix/csrc) —— 自由式 C 运行环境
- [`libline`](https://github.com/BRX-Boruix/libline) —— 行编辑库

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。

---

# English

[简体中文](#libsys) | **English**

BORUIX's **syscall wrapper layer** — the boundary between user-space programs and the kernel.

It defines the calling interface both sides agree on, wraps the raw invocation in a safe Rust-style
interface, and provides the entry point for user programs.

```
user program  →  libsys (this library)  →  kernel
```

---

## The problem it solves

To read a file, allocate memory, or create a thread, a user-space program must ask the kernel to do it
on its behalf. That request works by **convention**: the program places a call number and arguments
in designated registers, executes a special instruction, and the kernel dispatches accordingly.

Using that mechanism raw is possible but painful — you handle the register convention, check the
return value's error marker, and remember every call number. `libsys` gathers that into a thin
wrapper: the caller writes `read(fd, &mut buf)` and gets a `Result`, without caring what the
registers look like underneath.

**Why "thin" matters**: a good wrapper should not hide or alter semantics. Error codes, blocking
behaviour, and argument meanings should pass through faithfully. The moment a wrapper starts "handling
things for you", it becomes a second source of truth — and when it diverges from the kernel, the
truth of which lives in the kernel, the result is extremely hard to diagnose.

## Writing a user program

`libsys` provides the program entry point. A user program only exports a `user_main`:

```rust
#![no_std]
#![no_main]

use libsys::{write, STDOUT};

#[unsafe(no_mangle)]
pub extern "C" fn user_main(_argc: isize, _argv: *const *const u8) -> i32 {
    let _ = write(STDOUT, b"hello from userspace\n");
    0
}
```

The entry point takes the arguments off the stack, aligns the stack, calls `user_main`, and uses its
return value as the process exit code. **`user_main` is the only symbol a user program must export.**

## How syscalls are numbered

Call numbers are not merely incremented — they follow an **orthogonal scheme**, assembled from two
parts:

```
(resource domain << 4) | verb
```

A **resource domain** says what kind of thing is being operated on, and a **verb** says what is being
done:

| Resource domain | Meaning | Verb | Meaning |
| --- | --- | --- | --- |
| `STREAM` | byte streams | `CREATE` | create |
| `MEMORY` | memory | `READ` | read |
| `TASK` | processes and threads | `WRITE` | write |
| `VFS` | filesystems | `DELETE` | delete |
| `DEVICE` | devices | | |

The benefit is that **readability is encoded in the number itself**: the high bits say what is
operated on and the low bits say what is done, with no lookup table required. Further domains
(synchronisation, signals, power, audio, volume management) extend the same scheme.

## Making a call

A call enters the kernel through a special instruction. The convention is:

| Register | Purpose |
| --- | --- |
| `rax` | the syscall number |
| `rdi` / `rsi` / `rdx` / `r10` / `r8` / `r9` | the six arguments |

The last three arguments live in less common registers because ordinary compiler-generated code must
not occupy them — this is pinned explicitly at the assembly level, since otherwise the kernel would
read garbage from the saved frame.

## How errors are represented

After a call returns, the **top bit is the error marker**: set means failure, and the remaining bits
are the negated error code. The wrapper unpacks this into a Rust `Result`, so callers need not check
flags by hand.

Error codes are **shared with the kernel** — the same meaning must have the same value on both sides,
or callers will act on the wrong semantics.

## Modules

The library covers the domains of the syscall interface:

| Area | Contents |
| --- | --- |
| Call mechanism | call number definitions, the trap instruction, error unpacking |
| Input/output | opening, reading, writing, directory traversal, file attributes, permissions |
| Processes | creation, waiting, exiting, signals, job queries, identity and groups |
| Threads | creation, joining, exiting, synchronisation primitives |
| Memory | heap extension, memory mapping |
| Devices and drivers | device claiming, DMA allocation, interrupt waiting, topology events |
| Volume management | mounting, unmounting, listing, probing |
| Audio | audio stream control |
| Other | time, randomness, power management, JSON parsing, event translation |

## The event translation layer

The library includes a module that **turns input events into terminal bytes**: keycode mapping,
modifier state, and control key folding.

It lives in user space rather than the kernel by deliberate division of labour — **keycode layout is
policy, not mechanism**. The kernel only needs to hand over "which key was pressed" as it is, and
keeping layout and escape rules in user space means they can be adjusted without touching the kernel.
The kernel's input therefore remains a raw event stream.

## Who it serves

`libsys` **depends on no other BORUIX component**; it sits at the very bottom of user space. It
serves two kinds of consumer:

- **Rust programs** — depending on it directly
- **C programs** — reaching the same syscalls through [`libc`](https://github.com/BRX-Boruix/libc)
  (implemented in Rust, exposing C interfaces)

## Testing

The library carries **60 unit tests**, covering what can be verified without a device — error
unpacking, argument handling, event translation, JSON parsing, and more.

## Building

```bash
cargo build --release
cargo test
```

## Layout

```
libsys/
└── src/
    ├── lib.rs        # the public interface
    ├── nr.rs         # syscall number definitions
    ├── syscall.rs    # the trap entry and error unpacking
    ├── error.rs      # error codes
    ├── start.rs      # the user program entry point
    └── ...           # per-domain wrappers
```

## Related projects

- [`libc`](https://github.com/BRX-Boruix/libc) — the C standard library, built on this one
- [`csrc`](https://github.com/BRX-Boruix/csrc) — the freestanding C runtime
- [`libline`](https://github.com/BRX-Boruix/libline) — the line editing library

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
