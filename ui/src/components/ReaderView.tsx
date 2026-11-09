import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';

import type { PDFDocumentProxy } from 'pdfjs-dist';
import { downloadPdf } from '../api/ipc';
import { ipcErrorMessage } from '../lib/ipcError';
import { PDF_DOCUMENT_OPTIONS, pdfjs } from '../lib/pdfjs';
import { extractFullText, extractPage } from '../lib/pdfText';
import type { Paper, Summary } from '../types';
import AiPanel from './AiPanel';
import PdfPane from './PdfPane';

interface ReaderViewProps {
  paper: Paper;
  summary: Summary | null;
  summaryStream: string;
  summaryBusy: boolean;
  summaryError: string | null;
  onRetrySummary: () => void;
  onClose: () => void;
}

type LoadStatus = 'loading' | 'ready' | 'error';

const AI_MIN_WIDTH = 300;
const AI_DEFAULT_WIDTH = 380;
const AI_MIN_PDF_WIDTH = 420;
const AI_WIDTH_KEY = 'paperflow.reader.aiWidth.v1';

function clampAiWidth(width: number): number {
  const max = Math.max(AI_MIN_WIDTH, window.innerWidth - AI_MIN_PDF_WIDTH);
  return Math.min(Math.max(width, AI_MIN_WIDTH), max);
}

function loadAiWidth(): number {
  try {
    const stored = Number(window.localStorage.getItem(AI_WIDTH_KEY));
    if (Number.isFinite(stored) && stored > 0) return clampAiWidth(stored);
  } catch {
    // Storage may be unavailable; fall back to the default width.
  }
  return clampAiWidth(AI_DEFAULT_WIDTH);
}

function persistAiWidth(width: number): void {
  try {
    window.localStorage.setItem(AI_WIDTH_KEY, String(Math.round(width)));
  } catch {
    // Storage may be unavailable; the width still applies for this session.
  }
}

export default function ReaderView({
  paper,
  summary,
  summaryStream,
  summaryBusy,
  summaryError,
  onRetrySummary,
  onClose,
}: ReaderViewProps) {
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [numPages, setNumPages] = useState(0);
  const [page, setPage] = useState(1);
  const [scale, setScale] = useState(1);
  const [status, setStatus] = useState<LoadStatus>('loading');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectionText, setSelectionText] = useState('');
  const [reloadToken, setReloadToken] = useState(0);
  const [aiWidth, setAiWidth] = useState(loadAiWidth);
  const aiWidthRef = useRef(aiWidth);
  const dragRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const pageTextCache = useRef(new Map<number, string>());
  const fullTextRef = useRef<string | null>(null);
  const fullTextPromiseRef = useRef<Promise<string> | null>(null);

  useEffect(() => {
    let cancelled = false;
    let loaded: PDFDocumentProxy | null = null;
    let loadingTask: ReturnType<typeof pdfjs.getDocument> | null = null;

    setStatus('loading');
    setLoadError(null);
    setDoc(null);
    setNumPages(0);
    setPage(1);
    setScale(1);
    setSelectionText('');
    pageTextCache.current.clear();
    fullTextRef.current = null;
    fullTextPromiseRef.current = null;

    void (async () => {
      try {
        if (!paper.pdf_url) {
          throw new Error('这篇论文没有可用的 PDF 链接');
        }

        const raw = (await downloadPdf(paper.pdf_url)) as
          | ArrayBuffer
          | number[];
        if (cancelled) return;

        const data =
          raw instanceof ArrayBuffer
            ? new Uint8Array(raw.slice(0))
            : Uint8Array.from(raw);

        loadingTask = pdfjs.getDocument({
          data,
          ...PDF_DOCUMENT_OPTIONS,
        });
        loaded = await loadingTask.promise;

        if (cancelled) {
          void loadingTask.destroy();
          return;
        }

        setDoc(loaded);
        setNumPages(loaded.numPages);
        setStatus('ready');
      } catch (error: unknown) {
        if (cancelled) return;
        setStatus('error');
        setLoadError(ipcErrorMessage(error));
      }
    })();

    return () => {
      cancelled = true;
      void loadingTask?.destroy();
    };
  }, [paper.id, paper.pdf_url, reloadToken]);

  const clearSelection = useCallback(() => {
    setSelectionText('');
    window.getSelection()?.removeAllRanges();
  }, []);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      if (selectionText) {
        clearSelection();
        return;
      }
      onClose();
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [selectionText, clearSelection, onClose]);

  const handlePageText = useCallback((pageNumber: number, text: string) => {
    pageTextCache.current.set(pageNumber, text);
  }, []);

  const getPageText = useCallback(
    async (pageNumber: number) => {
      const cached = pageTextCache.current.get(pageNumber);
      if (cached !== undefined) return cached;
      if (!doc) throw new Error('PDF 尚未加载');

      const text = await extractPage(doc, pageNumber);
      pageTextCache.current.set(pageNumber, text);
      return text;
    },
    [doc],
  );

  const getFullText = useCallback(
    (onProgress: (done: number, total: number) => void) => {
      if (fullTextRef.current !== null) {
        return Promise.resolve(fullTextRef.current);
      }
      if (!doc) {
        return Promise.reject(new Error('PDF 尚未加载'));
      }
      if (!fullTextPromiseRef.current) {
        fullTextPromiseRef.current = extractFullText(doc, onProgress)
          .then((text) => {
            fullTextRef.current = text;
            return text;
          })
          .finally(() => {
            fullTextPromiseRef.current = null;
          });
      }
      return fullTextPromiseRef.current;
    },
    [doc],
  );

  useEffect(() => {
    const handleResize = () => {
      const next = clampAiWidth(aiWidthRef.current);
      aiWidthRef.current = next;
      setAiWidth(next);
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  const applyWidth = (width: number) => {
    const next = clampAiWidth(width);
    aiWidthRef.current = next;
    setAiWidth(next);
    return next;
  };

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    dragRef.current = { startX: event.clientX, startWidth: aiWidthRef.current };
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const moveResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    applyWidth(drag.startWidth - (event.clientX - drag.startX));
  };

  const endResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragRef.current) return;
    dragRef.current = null;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    persistAiWidth(aiWidthRef.current);
  };

  const resetAiWidth = () => {
    persistAiWidth(applyWidth(AI_DEFAULT_WIDTH));
  };

  const handleResizeKey = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const delta =
      event.key === 'ArrowLeft' ? 16 : event.key === 'ArrowRight' ? -16 : 0;
    if (delta === 0) return;

    event.preventDefault();
    persistAiWidth(applyWidth(aiWidthRef.current + delta));
  };

  return (
    <div
      className="fixed inset-0 z-20 flex flex-col bg-bg"
      role="dialog"
      aria-modal="true"
      aria-label="论文阅读器"
    >
      <header className="flex items-center gap-3 border-b border-border bg-bg-raised px-4 py-2.5">
        <span className="min-w-0 flex-1 truncate font-semibold">
          {paper.title}
        </span>
        {status === 'ready' && (
          <span className="text-xs text-fg-dim">
            第 {page} / {numPages} 页
          </span>
        )}
        <button
          className="rounded-md px-2 py-1 text-base text-fg-dim hover:bg-bg-hover hover:text-fg"
          onClick={onClose}
          aria-label="关闭阅读器"
        >
          ✕
        </button>
      </header>

      <div className="flex min-h-0 flex-1">
        <section className="flex min-h-0 min-w-0 flex-1 flex-col">
          {status === 'loading' && (
            <p className="px-4 py-6 text-[13px] text-fg-dim">
              正在下载并解析 PDF…
            </p>
          )}
          {status === 'error' && (
            <div className="m-5 rounded-lg border border-danger px-4 py-3.5 text-[13px] text-danger">
              <p>{loadError}</p>
              <button
                className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent"
                onClick={() => setReloadToken((token) => token + 1)}
              >
                重试
              </button>
            </div>
          )}
          {doc && (
            <PdfPane
              doc={doc}
              numPages={numPages}
              page={page}
              scale={scale}
              onPageChange={setPage}
              onScaleChange={setScale}
              onSelectionChange={setSelectionText}
              onPageText={handlePageText}
            />
          )}
        </section>

        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="调整 AI 面板宽度（双击恢复默认）"
          aria-valuemin={AI_MIN_WIDTH}
          aria-valuemax={Math.max(AI_MIN_WIDTH, window.innerWidth - AI_MIN_PDF_WIDTH)}
          aria-valuenow={Math.round(aiWidth)}
          tabIndex={0}
          className="group relative z-10 w-1.5 shrink-0 cursor-col-resize focus-visible:outline-none"
          onPointerDown={startResize}
          onPointerMove={moveResize}
          onPointerUp={endResize}
          onPointerCancel={endResize}
          onLostPointerCapture={endResize}
          onDoubleClick={resetAiWidth}
          onKeyDown={handleResizeKey}
        >
          <div className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-border transition-colors group-hover:bg-accent group-focus-visible:bg-accent" />
        </div>

        <div
          style={{ width: `${aiWidth}px` }}
          className="flex min-h-0 shrink-0 flex-col"
        >
          <AiPanel
            paper={paper}
            summary={summary}
            summaryStream={summaryStream}
            summaryBusy={summaryBusy}
            summaryError={summaryError}
            onRetrySummary={onRetrySummary}
            currentPage={page}
            pdfReady={status === 'ready'}
            selectionText={selectionText}
            onClearSelection={clearSelection}
            getPageText={getPageText}
            getFullText={getFullText}
          />
        </div>
      </div>
    </div>
  );
}
