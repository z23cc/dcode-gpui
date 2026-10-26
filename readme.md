# DCode (GPUI)

A rewrite of the DCode desktop client in Rust on [GPUI](https://www.gpui.rs) and
[GPUI Kit](https://gpui-kit.com). It talks to the existing DCode host (the TS daemon from
`z23cc/dcode`) over the host's WebSocket protocol; the host itself is not rewritten.

| Crate                 | What it is                                                     |
| --------------------- | -------------------------------------------------------------- |
| `crates/dcode-client` | The wire: host discovery, `/rpc` calls, `/stream` subscriptions |
| `crates/dcode-api`    | Typed host API                                                 |
| `crates/dcode-app`    | The GPUI desktop app                                           |
| `experiments/`        | Throwaway measurements, outside the workspace                  |

## Run

Needs [just](https://just.systems), a running host (DCode or DCode Beta) and Xcode's Metal
toolchain. `rust-toolchain.toml` pins Rust 1.99.1; GPUI does not build on 1.97.

```bash
just run      # the app
just probe    # headless, read-only check of the wire
just check    # everything a change must pass
just hooks    # run `just check` before each commit
```

The host is found through its `pid.json`: DCode Beta's (`~/.dcode/host/staging/pid.json`) by
default. Set `DCODE_PID_JSON=~/.dcode/host/pid.json` for the installed app's host.

Conventions, layering and the wire protocol's invariants are in [AGENTS.md](AGENTS.md).
