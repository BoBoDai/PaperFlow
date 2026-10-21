---
name: llm-provider
description: Add or debug an OpenAI-compatible LLM provider in PaperFlow, including profiles, capabilities, settings migration, write-only keys, and streaming behavior.
---

# LLM provider work

Use this when adding a provider preset, changing request compatibility, or debugging
why a configured model fails. Read `docs/architecture.md` section 7–8 first for the
current pipeline.

## Relevant files

| Concern | File |
|---|---|
| Provider enum, defaults, capabilities | `crates/adapters/src/llm/profiles.rs` |
| Runtime config, Debug redaction, URL validation | `crates/adapters/src/llm/config.rs` |
| Chat / SSE / model list / connection test | `crates/adapters/src/llm/client.rs` |
| SSE parsing and `<think>` filtering | `crates/adapters/src/llm/streaming.rs` |
| Prompts | `crates/adapters/src/llm/prompts.rs` |
| Strict output parsing | `crates/adapters/src/llm/parsing.rs` |
| Error mapping | `crates/adapters/src/llm/error.rs` |
| Persisted providers / migration | `src-tauri/src/settings/` |
| Settings UI | `ui/src/components/SettingsPanel.tsx` |
| Tests | `crates/adapters/src/llm/tests.rs`, `src-tauri/src/settings/tests.rs` |

## Add a provider preset

1. Add a variant to `ProviderProfileId` and update:
   - `as_str`
   - `from_id`
   - `defaults()`
   - `capabilities()`
   - `provider_catalog()`
2. Set capabilities deliberately:
   - `json_mode`: whether `response_format: {type: "json_object"}` is accepted.
   - `max_tokens_field`: `max_tokens` vs `max_completion_tokens`.
   - `thinking_mode` / `thinking_format`: omit (`Default`), disable, or adaptive;
     `thinking` vs `enable_thinking`.
3. Update `infer_provider` in `src-tauri/src/settings/migration.rs` if legacy
   configs should map to the new variant.
4. Check `SettingsPanel.tsx` renders the new catalog entry sensibly; add a label
   only if the catalog is not enough.
5. Add tests:
   - `llm/tests.rs`: default body shape / capability override.
   - `settings/tests.rs`: migration and provider inference.
6. Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

## Debug checklist

When a call fails, check in this order:

1. **Shape of the error**: `LlmEndpointError` carries `kind`, HTTP status, request id
   and provider message. The UI formats it with `formatLlmError`.
2. **Base URL**: must end at the version path (e.g. `.../v1`), not include
   `/chat/completions` or `/models`; `normalize_base_url` rejects those.
3. **Model id**: use `fetch_models` (`GET /models`) rather than guessing. Some
   gateways expose decorated ids.
4. **Token field**: if the provider returns 400 for `max_tokens`, switch the profile
   or compat override to `max_completion_tokens`.
5. **JSON mode**: some providers reject `response_format`; disable `json_mode` via
   the profile or `LlmCompat`.
6. **Thinking**: MiniMax M3.x rejects `thinking: {"type":"disabled"}`. Keep
   `ThinkingMode::Default` for MiniMax unless the user explicitly overrides it.
   `<think>` blocks are stripped before parsing.
7. **Empty response**: check `finish_reason` and token budget. Adaptive thinking can
   consume the entire budget before content; raise `max_tokens` and retry.
8. **Streaming**: if the gateway ignores `stream=true`, `stream_chat` falls back to
   parsing a normal completion. If SSE chunks are malformed they are logged and
   skipped; an empty final answer is `InvalidLlmOutput`.

## Security rules

- Never log or return an API key. `LlmConfig`'s `Debug` prints `***`.
- Keys live in `credentials.json`, not `settings.json`; the IPC view only exposes
  `has_api_key`.
- Blank key on save keeps the old key; `clear_api_key` removes it.
- Do not add a silent fallback that retries with a different provider or model.

## Verify

```bash
cargo test -p paperflow-adapters
cargo test -p paperflow-app
cargo clippy --workspace --all-targets -- -D warnings
pnpm typecheck
```

Then use the in-app settings panel: fetch models, test connection, save, and run one
real summary or Q&A.
