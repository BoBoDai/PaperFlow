---
name: pdf-reader
description: Work on PaperFlow's PDF download, PDF.js rendering, text layer, selection explanation, and page/full-paper Q&A.
---

# PDF reader work

Use this for download failures, rendering/worker issues, text selection, or Q&A
context problems. Read `docs/architecture.md` section 6 first.

## Pipeline

```
paper.pdf_url
  → ipc.downloadPdf(url)                 ui/src/api/ipc.ts
  → commands::download_pdf               src-tauri/src/commands.rs
  → PdfFetcher::fetch                    crates/adapters/src/pdf.rs
  → tauri::ipc::Response (ArrayBuffer)
  → pdfjs.getDocument({ data })          ui/src/lib/pdfjs.ts
  → ReaderView / PdfPane                 ui/src/components/*
  → extractPage / extractFullText        ui/src/lib/pdfText.ts
  → askPaper / askPaperStream            ui/src/api/ipc.ts
```

## Backend rules

`PdfFetcher` in `crates/adapters/src/pdf.rs`:

- HTTPS only; reject userinfo, single-label hosts, localhost, `.local`, `.internal`.
- Resolve DNS and require a public IP; re-check every redirect (max 5).
- 32 MiB cap while streaming; reject bodies without `%PDF-` in the first 1 KiB.
- Cache at `{app_cache_dir}/pdfs/{fnv1a64(url)}.pdf`; write temp + rename.
- Map 401/403, 404, 429/5xx to actionable Chinese errors.
- Never fetch the PDF from the WebView; that reintroduces CORS and leaks the URL.

`download_pdf` returns raw bytes. Do not JSON-encode them.

## Frontend rules

- `pdfjs-dist` legacy build; worker via `?url`; `cmaps` and `standard_fonts` are
  copied by `vite-plugin-static-copy` (see `ui/vite.config.ts`).
- `PDF_DOCUMENT_OPTIONS` sets `cMapUrl`, `standardFontDataUrl`, `isEvalSupported: false`.
- `ReaderView` owns the document and text caches; use `key={paper.id}` so switching
  papers does not leak state.
- `PdfPane` renders canvas + PDF.js text layer. The `textLayer` structural rules in
  `styles.css` are required; do not replace them with utilities.
- Selection: `window.getSelection()` → trimmed text → “解释选中内容”.
- Q&A scopes: selection (8k), current page (40k), full paper (40k; 30k head + 10k
  tail when truncated). History is trimmed by the usecase, not the UI.
- Stream via `Channel<string>`; show partial text immediately and keep errors out of
  chat history.
- Escape clears selection first, then closes the reader.

## Common failures

| Symptom | Check |
|---|---|
| “不是 PDF（可能是出版商登录/订阅页）” | URL returns HTML / 403; try another OA link |
| 401/403 | publisher paywall, not a code bug |
| 404 | stale `pdf_url` |
| 32 MiB limit | large PDF; do not silently raise the cap without discussing memory |
| Worker / MIME error | `workerSrc`, static copy output, dev vs production build |
| Blank page | canvas dimensions, viewport scale, render cancellation |
| No selectable text | scanned PDF; there is no OCR in v1 |
| Empty Q&A context | text extraction failed or scanned PDF; show the explicit message |
| Full-paper answer misses middle | expected; context is head + tail truncated, UI says so |

## Verify

Automatic:

```bash
cargo test -p paperflow-adapters
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm typecheck
pnpm build
```

Manual (`pnpm app:dev`):

1. Open a paper with a valid `pdf_url`; first page renders.
2. Page navigation, page number input, zoom, 100% reset.
3. Select text → explain → Chinese answer.
4. Current-page Q&A cites a page; full-paper Q&A shows truncation notice.
5. Follow-up question keeps recent history.
6. Open a second paper; no state from the first leaks.
7. Test one failure: missing URL, 403 publisher, or scanned PDF.
8. Verify the production build if worker/cmap paths changed: `pnpm app:build`.
