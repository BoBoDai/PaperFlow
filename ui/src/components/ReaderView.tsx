import { useCallback, useEffect, useRef, useState } from 'react';

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

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(340px,420px)] max-[900px]:grid-cols-[minmax(0,1fr)_340px]">
        <section className="flex min-h-0 min-w-0 flex-col border-r border-border">
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
  );
}
