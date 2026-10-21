---
name: add-ipc-command
description: Add a new Tauri IPC command end-to-end in PaperFlow, from domain types and usecase tests through command registration and a typed React wrapper.
---

# Add an IPC command

Use this when a new frontend feature needs backend data or an action that does not
already have a command. Read `CLAUDE.md` and `docs/architecture.md` first if this is
a new session.

## Read the closest existing example

- Simple command: `src-tauri/src/commands.rs::summarize_paper`
- Streaming command: `commands.rs::ask_paper_stream`
- Raw bytes: `commands.rs::download_pdf`
- Frontend wrapper: `ui/src/api/ipc.ts`
- App wiring: `src-tauri/src/lib.rs::run`

## Steps

1. **Domain types** (only if new shapes are needed)
   - Add them to `crates/core/src/` and export from `crates/core/src/lib.rs`.
   - Derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` as appropriate.
   - Mirror the shape in `ui/src/types.ts`; Rust is the source of truth.

2. **Usecase**
   - Put orchestration in `crates/usecases/src/` (search / feed / qa / lib, or a new
     sibling module for a distinct usecase).
   - Keep it transport-agnostic: no `tauri::`, no `axum::`, no IPC types.
   - Return `paperflow_core::Result<T>`.
   - Add a unit test under `crates/usecases/src/tests/` using `FakeSearch` / `FakeLlm`
     from `tests/support.rs` where possible. No network in default tests.

3. **Command**
   - Add a thin `#[tauri::command]` in `src-tauri/src/commands.rs`:

     ```rust
     #[tauri::command]
     pub async fn my_command(
         state: State<'_, AppState>,
         input: InputType,
     ) -> IpcResult<OutputType> {
         state.app.my_usecase(input).await.map_err(to_ipc)
     }
     ```

   - For streaming, accept `on_event: tauri::ipc::Channel<String>`, clone it, pass a
     `move |delta| { let _ = channel.send(delta); }` callback, and return the final
     response from the command.
   - For binary data, return `tauri::ipc::Response::new(bytes)`.
   - No business logic, no retries, no fallback, no silent empty result here.

4. **Register**
   - Add the command to `tauri::generate_handler!` in `src-tauri/src/lib.rs`.
   - Missing registration compiles but fails at runtime with “command not found”.

5. **Frontend**
   - Add a typed wrapper in `ui/src/api/ipc.ts`; components never call `invoke`.
   - Use `Channel` only for streaming commands:

     ```ts
     const channel = new Channel<string>();
     channel.onmessage = onDelta;
     return invoke<T>("my_stream_command", { input, onEvent: channel });
     ```

   - Catch errors with `ipcErrorMessage` / `formatLlmError` from `ui/src/lib/ipcError.ts`.

6. **UI**
   - Wire the wrapper into `App.tsx` or the relevant component.
   - Keep separate loading / error state per area; do not reuse one global error.
   - If a new field is persisted or shown, update `ui/src/types.ts` and any fixtures.

## Conventions

- Command *argument names* are camelCase on the JS side by default (`onEvent` maps to
  `on_event`). Domain struct fields keep their serde shape; current `ui/src/types.ts`
  mirrors them as snake_case. Do not mix the two up.
- Errors cross IPC as `String`; log once with `tracing::error!` in `to_ipc`.
- API keys must never be returned by a command or written to logs.
- If the command changes a prompt, bump the relevant `PROMPT_VERSION`.

## Verify

Run from the repo root:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm typecheck
pnpm build
```

Then run `pnpm app:dev` and exercise the new path manually, including one error case.
