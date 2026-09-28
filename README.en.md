# libsys

BORUIX's **user-space system call library**: it wraps the system calls the kernel provides into Rust functions, for every user-space program to use.

[简体中文](README.md)

## What it does

A user-space program cannot issue a system call directly — it must pass arguments, trigger the call, and check the result the agreed way. This library wraps that convention:

```rust
use libsys::{open, read, write, close};
```

## Coverage

**101 system calls** are wrapped, grouped into modules by domain:

| Domain | Contents |
| --- | --- |
| **Files and I/O** | Open, read, write, close, directory operations, pipes |
| **Memory** | Heap growth, mapping and unmapping |
| **Process** | Spawn, wait, exit, identity query and switching, process groups |
| **Threads** | Spawn, join, exit, yield |
| **Synchronisation** | Create, delete, wait, wake |
| **Signals** | Send, install handlers, mask |
| **Time** | Monotonic clock, wall clock, sleep |
| **Devices and volumes** | Device events, volume enumeration, mount and unmount |
| **Drivers** | Device registration, claim, mapping, query |
| **Audio** | Attach consumers, fetch frames, commit |
| **Other** | Random numbers, JSON parsing, system information, power off and reboot |

## Usage

Referenced as a dependency by other user-space programs; it is not usually run on its own:

```toml
[dependencies]
libsys = { path = "../libsys" }
```

The library registers a user-space **heap allocator**, so `Vec`, `String`, `Box`, and the other standard collections can be used directly. The heap is requested from the kernel on first allocation and grows on demand.

## Building

```bash
cargo build --release
```

## Layout

```
libsys/src/
├── lib.rs        # module exports and re-exports
├── nr.rs         # system call number definitions
├── syscall.rs    # issuing system calls and translating errors
├── error.rs      # error type
├── start.rs      # program entry point
├── allocator.rs  # heap allocator
├── io.rs         # files and I/O
├── mem.rs        # memory
├── process.rs    # processes
├── thread.rs     # threads
├── sync.rs       # synchronisation
├── signal.rs     # signals
├── time.rs       # time
├── volume.rs     # devices and volumes
├── driver.rs     # drivers
├── audio.rs      # audio
├── event.rs      # event streams and keymaps
├── pipe.rs       # pipes
├── power.rs      # power off and reboot
├── random.rs     # random numbers
├── json.rs       # JSON parsing
├── object.rs     # object verbs (streams, sync, tasks, filesystem)
├── system.rs     # system information
└── builtins.rs   # built-in helpers
```

## Related projects

- [`libc`](https://github.com/BRX-Boruix/libc) — the C standard library interfaces, built on this library
- [`libline`](https://github.com/BRX-Boruix/libline) — the line editing library, which uses this library's input interfaces
- [`selftest`](https://github.com/BRX-Boruix/selftest) — the host program for end-to-end acceptance

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
