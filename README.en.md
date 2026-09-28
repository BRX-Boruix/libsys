# libsys

BORUIX's user-space system call library. A user-space program cannot issue a system call directly — it must pass arguments, trigger the call, and check the result the agreed way. This library wraps that convention into Rust functions.

[简体中文](README.md)

## Coverage

**101 system calls** are wrapped, across these areas:

- Files and I/O: open, read, write, close, directory operations, pipes
- Memory: heap growth, mapping and unmapping
- Processes and threads: spawn, wait, exit, identity query and switching, join, yield
- Synchronisation and signals: create, delete, wait, wake, send, install handlers, mask
- Time: monotonic clock, wall clock, sleep
- Devices, volumes, and drivers: device events, volume enumeration and mounting, device registration and claiming
- Audio: attaching consumers, fetching frames, committing
- Other: random numbers, JSON parsing, system information, power off and reboot

## Usage

Referenced as a dependency by other user-space programs; it is not usually run on its own:

```toml
[dependencies]
libsys = { path = "../libsys" }
```

The library registers a user-space heap allocator, so `Vec`, `String`, `Box`, and the other standard collections can be used directly. The heap is requested from the kernel on first allocation and grows on demand.

## Building

```bash
cargo build --release
```

## Repository layout

- `src/lib.rs` — module exports and re-exports
- `src/nr.rs` — system call number definitions
- `src/syscall.rs` — issuing system calls and translating errors
- `src/start.rs` — program entry point
- `src/allocator.rs` — heap allocator
- `src/io.rs`, `mem.rs`, `process.rs`, `thread.rs`, `sync.rs`, `signal.rs`, `time.rs` — the interfaces for each area
- `src/volume.rs`, `driver.rs`, `audio.rs`, `event.rs` — devices, drivers, and audio
- `src/pipe.rs`, `power.rs`, `random.rs`, `json.rs`, `object.rs`, `system.rs` — the remaining interfaces

## Related projects

- [`libc`](https://github.com/BRX-Boruix/libc) — the C standard library interfaces, built on this library
- [`libline`](https://github.com/BRX-Boruix/libline) — the line editing library, which uses this library's input interfaces
- [`selftest`](https://github.com/BRX-Boruix/selftest) — the host program for end-to-end acceptance

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
