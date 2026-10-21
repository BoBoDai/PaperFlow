import { useEffect, useRef, useState } from 'react';

import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';
import { pdfjs } from '../lib/pdfjs';
import { extractPageText } from '../lib/pdfText';
import { ipcErrorMessage } from '../lib/ipcError';

interface PdfPaneProps {
  doc: PDFDocumentProxy;
  numPages: number;
  page: number;
  scale: number;
  onPageChange: (page: number) => void;
  onScaleChange: (scale: number) => void;
  onSelectionChange: (text: string) => void;
  onPageText: (page: number, text: string) => void;
}

const MIN_SCALE = 0.6;
const MAX_SCALE = 2.5;
const SCALE_STEP = 0.15;

function clampScale(scale: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

export default function PdfPane({
  doc,
  numPages,
  page,
  scale,
  onPageChange,
  onScaleChange,
  onSelectionChange,
  onPageText,
}: PdfPaneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const textLayerRef = useRef<HTMLDivElement | null>(null);
  const pageRef = useRef<HTMLDivElement | null>(null);
  const renderTaskRef = useRef<RenderTask | null>(null);

  const [pageInput, setPageInput] = useState(String(page));
  const [rendering, setRendering] = useState(true);
  const [renderError, setRenderError] = useState<string | null>(null);

  useEffect(() => {
    setPageInput(String(page));
  }, [page]);

  useEffect(() => {
    let cancelled = false;
    let textLayer: InstanceType<typeof pdfjs.TextLayer> | null = null;

    setRendering(true);
    setRenderError(null);

    void (async () => {
      try {
        const pdfPage = await doc.getPage(page);
        if (cancelled) return;

        const viewport = pdfPage.getViewport({ scale });
        const canvas = canvasRef.current;
        const layer = textLayerRef.current;
        if (!canvas || !layer) return;

        const ratio = window.devicePixelRatio || 1;
        canvas.width = Math.floor(viewport.width * ratio);
        canvas.height = Math.floor(viewport.height * ratio);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;

        const context = canvas.getContext('2d');
        if (!context) throw new Error('无法创建 PDF 画布');

        renderTaskRef.current?.cancel();
        const renderTask = pdfPage.render({
          canvas,
          canvasContext: context,
          viewport,
          ...(ratio !== 1
            ? { transform: [ratio, 0, 0, ratio, 0, 0] }
            : {}),
        });
        renderTaskRef.current = renderTask;
        await renderTask.promise;

        if (cancelled) return;

        const textContent = await pdfPage.getTextContent();
        if (cancelled) return;

        layer.replaceChildren();
        layer.style.setProperty('--scale-factor', String(scale));
        layer.style.setProperty('--total-scale-factor', String(scale));
        textLayer = new pdfjs.TextLayer({
          textContentSource: textContent,
          container: layer,
          viewport,
        });
        await textLayer.render();

        onPageText(
          page,
          extractPageText(
            textContent.items as unknown as Array<{ str?: string; hasEOL?: boolean }>,
          ),
        );
      } catch (error: unknown) {
        if (cancelled) return;
        if (error instanceof Error && error.name === 'RenderingCancelledException') {
          return;
        }
        setRenderError(ipcErrorMessage(error));
      } finally {
        if (!cancelled) setRendering(false);
      }
    })();

    return () => {
      cancelled = true;
      renderTaskRef.current?.cancel();
      textLayer?.cancel();
    };
  }, [doc, page, scale, onPageText]);

  useEffect(() => {
    onSelectionChange('');
    window.getSelection()?.removeAllRanges();
  }, [page, scale, onSelectionChange]);

  const commitPageInput = () => {
    const value = Number(pageInput);
    if (Number.isInteger(value) && value >= 1 && value <= numPages) {
      onPageChange(value);
    } else {
      setPageInput(String(page));
    }
  };

  const handleSelection = () => {
    const selection = window.getSelection();
    const range =
      selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
    const text = selection?.toString().replace(/\s+/g, ' ').trim() ?? '';

    if (
      !text ||
      !range ||
      !pageRef.current?.contains(range.commonAncestorContainer)
    ) {
      onSelectionChange('');
      return;
    }

    onSelectionChange(text.slice(0, 20_000));
  };

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2 text-xs">
        <button
          className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
          onClick={() => onPageChange(page - 1)}
          disabled={page <= 1}
        >
          上一页
        </button>
        <input
          className="w-[52px] rounded-md border border-border bg-bg px-1.5 py-1 text-center text-fg outline-none focus:border-accent"
          value={pageInput}
          onChange={(event) =>
            setPageInput(event.target.value.replace(/[^\d]/g, ''))
          }
          onBlur={commitPageInput}
          onKeyDown={(event) => {
            if (event.key === 'Enter') commitPageInput();
          }}
          aria-label="页码"
        />
        <span className="text-xs text-fg-dim">/ {numPages}</span>
        <button
          className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
          onClick={() => onPageChange(page + 1)}
          disabled={page >= numPages}
        >
          下一页
        </button>

        <span className="flex-1" />

        <button
          className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
          onClick={() => onScaleChange(clampScale(scale - SCALE_STEP))}
          disabled={scale <= MIN_SCALE}
        >
          −
        </button>
        <button
          className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
          onClick={() => onScaleChange(1)}
          title="重置缩放"
        >
          {Math.round(scale * 100)}%
        </button>
        <button
          className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
          onClick={() => onScaleChange(clampScale(scale + SCALE_STEP))}
          disabled={scale >= MAX_SCALE}
        >
          ＋
        </button>
      </div>

      {renderError && (
        <div className="mx-3 my-2 rounded-lg border border-danger px-3 py-2 text-[12.5px] text-danger">
          {renderError}
        </div>
      )}

      <div
        className="flex flex-1 items-start justify-center overflow-auto bg-[#0b0d12] p-4"
        onMouseUp={handleSelection}
        onKeyUp={handleSelection}
      >
        <div
          className="relative flex-none bg-white shadow-[0_6px_24px_rgba(0,0,0,0.45)]"
          ref={pageRef}
        >
          <canvas className="block" ref={canvasRef} />
          <div className="textLayer" ref={textLayerRef} />
          {rendering && (
            <div className="absolute inset-0 grid place-items-center bg-white/70 text-xs text-[#444]">
              渲染中…
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
