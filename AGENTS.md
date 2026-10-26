# AGENTS.md

DCode's desktop client, rewritten in Rust on GPUI and gpui-kit. It talks to the existing
TypeScript DCode host over the host's WebSocket protocol; the host is not part of this repo.

Run `just check` before you call any change done. It must pass with no new `#[expect]`
that a reviewer would reject.

## Commands

| Command                    | What it does                                                      |
| -------------------------- | ----------------------------------------------------------------- |
| `just check`               | fmt check, clippy (`-D warnings`), tests, source lint             |
| `just fix`                 | clippy `--fix`, then fmt                                          |
| `just run`                 | the app (needs a running host)                                    |
| `just probe`               | read-only live check of the wire against a running host           |
| `just manifest <checkout>` | regenerate the handshake manifest from a TS `z23cc/dcode` checkout |
| `just hooks`               | run `just check` before every commit                              |

## Layout

Dependencies point one way: `dcode-app` → `dcode-api` → `dcode-client`. Nothing below imports
anything above it.

| Path                                  | Owns                                                               |
| ------------------------------------- | ------------------------------------------------------------------ |
| `crates/dcode-client`                 | The wire: host discovery, `/rpc` calls, `/stream` subscriptions    |
| `crates/dcode-api`                    | Typed host API; request and response shapes live only in `src/wire.rs` |
| `crates/dcode-app/src/app.rs`         | Bootstrap: windows, app-lifetime handlers                          |
| `crates/dcode-app/src/state/`         | Entities holding data and async work; no rendering                 |
| `crates/dcode-app/src/views/`         | `Render` views that compose components and read state              |
| `crates/dcode-app/src/components/`    | Stateless `RenderOnce` components; no entities, no I/O             |
| `experiments/`                        | Throwaway measurements; outside the workspace and these rules       |

Where new code goes:

| You are adding                         | Put it in                                                  |
| -------------------------------------- | ---------------------------------------------------------- |
| A host RPC or stream method            | `dcode-api`: params and result in `wire.rs`, function in a topic module |
| Transport behaviour (frames, timeouts) | `dcode-client`                                             |
| Data a screen loads or edits           | `dcode-app/src/state/<topic>.rs`                           |
| A screen or panel                      | `dcode-app/src/views/<screen>.rs`                          |
| A reusable piece of UI                 | `dcode-app/src/components/<name>.rs`                       |

Reference implementations to copy: `state/home.rs`, `views/home.rs`,
`components/sidebar_item.rs`, `dcode-api/src/epics.rs`, `dcode-client/tests/wire.rs`.

## Enforced by tools

- `rustfmt.toml`: width 100.
- Clippy `pedantic`, plus `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`,
  `dbg_macro`, `print_stdout`, `print_stderr`, `mod_module_files`, `allow_attributes`,
  `allow_attributes_without_reason`, `undocumented_unsafe_blocks`; `unsafe_code` is denied.
- `clippy.toml`: at most 60 lines per function and 5 arguments. `unwrap`/`expect`/`panic`/
  `print` are allowed inside `#[test]` functions and `#[cfg(test)]` modules only.
- `scripts/lint-source.sh`: at most 400 lines per `.rs` file; no comments except `// SAFETY:`
  (line, trailing and block comments all count).
- `unreachable_pub`: an item is `pub` only if it is reachable from its crate's root.
- Workspace-wide allows: the doc lints (this repo has no doc comments) and
  `used_underscore_binding` (GPUI entities own tasks and subscriptions in `_` fields).

Never write `#[allow]`. When a lint is wrong at one site, use
`#[expect(clippy::lint, reason = "why this site is the exception")]`. That reason is the only
sanctioned inline explanation.

## Code

- No comments and no doc comments. Intent goes into names, types and named constants; a
  constraint that names cannot carry goes into a test, or into this file.
- Visibility is `pub(crate)` unless the item is part of a library crate's API.
- Modules are `name.rs` next to a `name/` directory; never `mod.rs`.
- Library crates define one `thiserror` `Error` enum and `pub type Result<T, E = Error>`.
  The app uses `anyhow` and shows errors with `{error:#}`.
- No `unwrap`, `expect` or `panic` outside tests. Return errors. `#[expect(clippy::expect_used,
  reason = ...)]` is for an invariant a test proves, or for a startup step the app cannot run
  without (the main window).
- Do not reuse the names of GPUI types (`Subscription`, `Task`, `Entity`, `Context`, ...) for
  your own types: `use gpui_kit::*` brings them into every UI module.
- `serde_json::Value` stays inside `dcode-client` and `dcode-api/src/wire.rs`. Everything above
  works with typed structs.
- Async code is runtime-agnostic (`futures-io`, `async-io`, `futures-lite`); never tokio.
  Every network wait has a timeout.
- Add a dependency only when the standard library, an existing dependency or a few lines
  cannot do the job.

## Design

- Extract shared code on its second use, not before. A trait needs two real implementations;
  test doubles do not count. No empty modules or directories "for later".
- Components are builders: `new(required)` plus chained setters, consumed by `render`.
- Render is a pure function of state. Loading, parsing and decisions live in `state/` or below.
- Mutate an entity only through methods on its own type, e.g. `state.update(cx, HomeState::refresh)`.
- Delete code that nothing uses.

## GPUI

- An entity owns its async work in a field such as `_fetch: Option<Task<()>>`; assigning a
  new task drops and cancels the old one. Never `.detach()` a task or subscription that
  touches UI; only app-lifetime handlers in `app.rs` may detach.
- Do I/O with `cx.background_spawn`, then write back with
  `cx.spawn(async move |this, cx| ...)` and `this.update(cx, ...)`; call `cx.notify()` after
  any change that affects rendering.
- Keep `cx.observe` / `cx.subscribe` results in `_subscriptions: Vec<Subscription>`. Use
  `EventEmitter` for events between entities; `Global` only for app-wide services.
- Split `render` into `render_*` methods that return concrete types (`Div`, `Stateful<Div>`).
  Under edition 2024 an `impl IntoElement` return captures `cx` and blocks the next call.
- Colors come only from `cx.theme()`. Sizes use rems (`p_2`, `text_sm`, `rems(..)`) so the UI
  scales; `px` is for hairlines and fixed window geometry.
- Element ids are stable and unique among siblings, e.g. `("epic", ix)`.
- A component with an id ends its element chain with `.test_support()` (a no-op outside
  tests), so tests can find and click it by that id.
- A list that can grow without bound is virtualized (`uniform_list` / `VirtualList`), never
  `.children(iter)`. `.children` is fine for a list capped by a page-size constant, like the
  epic sidebar (`EPIC_PAGE_SIZE`).
- Identify list items by their domain id, not their position: selection and element ids must
  survive a refresh that reorders the list (`HomeState::select`, `selected_epic`).
- The backend is GPUI Fast: root `Cargo.toml` `[workspace.dependencies]` enables `gpui-fast`
  on `gpui-kit`, and every crate takes `gpui-kit.workspace = true`. Never name a `gpui-kit`
  version or backend feature in a crate. Only GPUI Fast draws GPUI overlays above a WebView and
  clips it to its container; a WebView crate (`gpui-webview`) must enable its own `gpui-fast`
  feature to match.

## Tests

- Unit tests go in `#[cfg(test)] mod tests` at the bottom of the file; cross-module tests go in
  `crates/<crate>/tests/`.
- Name a test after the behaviour it pins: `call_surfaces_fatal_error_on_open`.
- Helpers outside `#[test]` functions return `Result` and use `?`; they never `unwrap`.
- No mocking frameworks. Fake at the process boundary: an in-process host on loopback
  (`dcode-client/tests/wire.rs`), JSON fixtures for wire shapes (`dcode-api`).
- GPUI views and components are tested with gpui-kit's `test-support` feature,
  `#[gpui_kit::test]` and `TestWindowExt` (find and click by element id); the template is the
  test module in `components/sidebar_item.rs`.
- A bug fix lands with a test that fails without it. `just test` never needs a running host
  or the network; `just probe` is the live check.

## Wire protocol invariants

The contract lives in the TS checkout (`protocol/src/framework/ws-protocol.ts`,
`stream-ws-protocol.ts`). Keep these true; each has, or needs, a test in `dcode-client`.

- The `open` frame's manifest must be exactly the host's floor method set; regenerate
  `crates/dcode-client/manifest.json` with `just manifest` whenever the host registry changes.
- The host gates on `clientIdentity.compatibilityEpoch`; it mirrors
  `CURRENT_CLIENT_COMPATIBILITY_EPOCH` in `protocol/src/framework/client-identity.ts`
  (currently 2). `kind` is diagnostic.
- Every method is version 1.0, so the client hard-codes 1.0 and skips the TS client's mirror
  compatibility check. A test over the embedded manifest fails when a method gets another
  version; add the mirror check then.
- One unary call per `/rpc` socket; one subscription per `/stream` socket. The stream URL is
  the RPC URL with `/rpc` replaced by `/stream`.
- The host pings each stream every 25 s and drops it after 60 s without a `pong`. The pong is
  sent only while `HostStream::next` is being awaited, so a consumer drives `next`
  continuously in its own task.
- `HostStream::next` is cancel-safe: an envelope whose binary payload has not arrived stays
  on the struct.
- Every wait is bounded: 10 s for the handshake, each send and each close, 60 s for a
  response, and 85 s of stream silence (one ping interval plus the pong deadline). Only the
  `within` mechanism is tested; the send, close and silence bounds are not, because their
  limits are constants. Keep the `within` wrappers in `send`, `close` and `HostStream::next`.
- Host messages exceed tungstenite's defaults: no frame cap, 1 GiB per message.

## Host API notes

- `epic.listTasks` params copy the TS GUI's `LIST_CLOUD_TASKS_REQUEST`
  (`clients/gui-app/src/lib/cloud-epic-tasks-query/query.ts`). The host's schema requires the
  two `extension*Version` fields but nothing reads them.

## Git

Do not commit unless asked. Never import from `experiments/`.
