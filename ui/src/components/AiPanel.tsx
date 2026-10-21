import { useEffect, useRef, useState } from 'react';

import { askPaperStream } from '../api/ipc';
import { ipcErrorMessage } from '../lib/ipcError';
import type { Paper, QaScope, QaTurn, Summary } from '../types';

interface AiPanelProps {
  paper: Paper;
  summary: Summary | null;
  summaryStream: string;
  summaryBusy: boolean;
  summaryError: string | null;
  onRetrySummary: () => void;
  currentPage: number;
  pdfReady: boolean;
  selectionText: string;
  onClearSelection: () => void;
  getPageText: (page: number) => Promise<string>;
  getFullText: (
    onProgress: (done: number, total: number) => void,
  ) => Promise<string>;
}

type Tab = 'chat' | 'summary';

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  display?: string;
  scope: QaScope;
  page: number | null;
  error?: boolean;
  contextTruncated?: boolean;
  contextChars?: number;
  model?: string;
  streaming?: boolean;
}

let messageSequence = 0;

function messageId(): string {
  messageSequence += 1;
  return `${Date.now()}-${messageSequence}`;
}

export default function AiPanel({
  paper,
  summary,
  summaryStream,
  summaryBusy,
  summaryError,
  onRetrySummary,
  currentPage,
  pdfReady,
  selectionText,
  onClearSelection,
  getPageText,
  getFullText,
}: AiPanelProps) {
  const [tab, setTab] = useState<Tab>('chat');
  const [scope, setScope] = useState<'current_page' | 'full_paper'>(
    'current_page',
  );
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [extracting, setExtracting] = useState<{
    done: number;
    total: number;
  } | null>(null);

  const requestIdRef = useRef(0);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (selectionText) setTab('chat');
  }, [selectionText]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [messages]);

  const send = async (raw: string, mode: 'typed' | 'selection') => {
    if (busy) return;

    const question =
      mode === 'selection'
        ? '请解释这段选中内容的含义和在论文中的作用。'
        : raw.trim();
    if (!question) return;

    const requestId = ++requestIdRef.current;
    const history: QaTurn[] = messages
      .filter((message) => !message.error && !message.streaming)
      .slice(-6)
      .map(({ role, content }) => ({ role, content }));

    let qaScope: QaScope;
    let page: number | null = currentPage;

    if (mode === 'selection') {
      qaScope = 'selection';
    } else if (scope === 'current_page') {
      qaScope = 'current_page';
    } else {
      qaScope = 'full_paper';
      page = null;
    }

    const userMessage: ChatMessage = {
      id: messageId(),
      role: 'user',
      content: question,
      display: mode === 'selection' ? '解释选中内容' : question,
      scope: qaScope,
      page,
    };

    const assistantId = messageId();
    setBusy(true);
    setTab('chat');
    setMessages((current) => [
      ...current,
      userMessage,
      {
        id: assistantId,
        role: 'assistant',
        content: '',
        scope: qaScope,
        page,
        streaming: true,
      },
    ]);

    try {
      let context: string;

      if (mode === 'selection') {
        context = selectionText;
      } else if (scope === 'current_page') {
        context = await getPageText(currentPage);
      } else {
        context = await getFullText((done, total) =>
          setExtracting({ done, total }),
        );
      }

      if (!context.trim()) {
        throw new Error(
          '没有可用的 PDF 文本。该 PDF 可能是扫描版，或文本尚未提取完成。',
        );
      }

      const response = await askPaperStream(
        {
          paper,
          question,
          scope: qaScope,
          context,
          page,
          history,
        },
        (delta) => {
          if (requestId !== requestIdRef.current) return;
          setMessages((current) =>
            current.map((message) =>
              message.id === assistantId
                ? { ...message, content: message.content + delta }
                : message,
            ),
          );
        },
      );

      if (requestId !== requestIdRef.current) return;

      setMessages((current) =>
        current.map((message) =>
          message.id === assistantId
            ? {
                ...message,
                content: response.answer,
                scope: response.scope,
                page: response.page,
                contextTruncated: response.context_truncated,
                contextChars: response.context_chars,
                model: response.model,
                streaming: false,
              }
            : message,
        ),
      );

      if (mode === 'selection') onClearSelection();
      setDraft('');
    } catch (error: unknown) {
      if (requestId !== requestIdRef.current) return;

      setMessages((current) =>
        current.map((message) =>
          message.id === assistantId
            ? {
                ...message,
                content: ipcErrorMessage(error),
                error: true,
                streaming: false,
              }
            : message,
        ),
      );
    } finally {
      if (requestId === requestIdRef.current) {
        setBusy(false);
        setExtracting(null);
      }
    }
  };

  return (
    <aside className="flex min-h-0 flex-col bg-bg-raised">
      <div className="flex border-b border-border">
        <button
          className={`flex-1 border-b-2 px-3 py-2.5 text-[13px] ${
            tab === 'chat'
              ? 'border-accent text-fg'
              : 'border-transparent text-fg-dim'
          }`}
          onClick={() => setTab('chat')}
        >
          问答
        </button>
        <button
          className={`flex-1 border-b-2 px-3 py-2.5 text-[13px] ${
            tab === 'summary'
              ? 'border-accent text-fg'
              : 'border-transparent text-fg-dim'
          }`}
          onClick={() => setTab('summary')}
        >
          分析
        </button>
      </div>

      {tab === 'summary' ? (
        <div className="flex-1 overflow-y-auto px-4 py-3.5">
          {summaryBusy && !summaryStream && (
            <p className="px-4 py-6 text-[13px] text-fg-dim">分析中…</p>
          )}
          {summaryBusy && summaryStream && (
            <p className="mb-2.5 whitespace-pre-wrap">{summaryStream}</p>
          )}
          {!summaryBusy && !summary && summaryStream && (
            <p className="mb-2.5 whitespace-pre-wrap">{summaryStream}</p>
          )}
          {!summaryBusy && summaryError && (
            <>
              <div className="mb-3.5 rounded-lg border border-danger px-3.5 py-2.5 text-[13px] text-danger">
                {summaryError}
              </div>
              <button
                className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
                onClick={onRetrySummary}
              >
                重试分析
              </button>
            </>
          )}
          {!summaryBusy && !summaryError && summary && (
            <>
              <p className="mb-2.5 whitespace-pre-wrap">{summary.short}</p>
              <p className="mb-2.5 whitespace-pre-wrap">{summary.detailed}</p>
              <ul className="m-0 pl-5">
                {summary.key_points.map((point, index) => (
                  <li key={index} className="mb-1">
                    {point}
                  </li>
                ))}
              </ul>
              <p className="m-0 text-xs text-fg-dim">
                模型 {summary.model} · prompt {summary.prompt_version}
              </p>
            </>
          )}
          {!summaryBusy && !summaryError && !summary && (
            <p className="px-4 py-6 text-[13px] text-fg-dim">
              尚未生成分析。
            </p>
          )}
        </div>
      ) : (
        <>
          <div className="flex flex-1 flex-col gap-3 overflow-y-auto px-4 py-3.5">
            <div className="flex items-center justify-between gap-2 border-b border-border pb-2">
              <div className="flex gap-1">
                <button
                  className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent aria-pressed:border-accent aria-pressed:text-accent disabled:opacity-45"
                  aria-pressed={scope === 'current_page'}
                  onClick={() => setScope('current_page')}
                >
                  当前页
                </button>
                <button
                  className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent aria-pressed:border-accent aria-pressed:text-accent disabled:opacity-45"
                  aria-pressed={scope === 'full_paper'}
                  onClick={() => setScope('full_paper')}
                >
                  全文
                </button>
              </div>
              <span className="text-xs text-fg-dim">
                {scope === 'current_page'
                  ? `第 ${currentPage} 页`
                  : '全文（超长时截断）'}
              </span>
            </div>

            <div className="flex flex-col gap-2.5">
              {messages.length === 0 && (
                <p className="px-4 py-6 text-[13px] text-fg-dim">
                  基于 PDF 内容提问。选中文字后可以让 AI 解释。
                </p>
              )}

              {messages.map((message) => (
                <div
                  key={message.id}
                  className={`max-w-[92%] whitespace-pre-wrap break-words rounded-[10px] border px-2.5 py-2 text-[13px] leading-relaxed select-text ${
                    message.role === 'user'
                      ? 'self-end bg-bg-hover'
                      : 'self-start bg-bg'
                  } ${message.error ? 'border-danger text-danger' : 'border-border'}`}
                >
                  {message.display ?? message.content}
                  {message.streaming && !message.content && (
                    <span className="text-fg-dim">思考中…</span>
                  )}
                  {message.streaming && message.content && (
                    <span className="animate-[chat-blink_1s_steps(1)_infinite] text-accent">
                      ▍
                    </span>
                  )}
                  {message.contextTruncated && (
                    <div className="mt-1.5 text-[11px] text-fg-dim">
                      上下文较长，已省略中间部分（发送 {message.contextChars}{' '}
                      字符）
                    </div>
                  )}
                  {message.model && (
                    <div className="mt-1.5 text-[11px] text-fg-dim">
                      模型 {message.model}
                    </div>
                  )}
                </div>
              ))}
              <div ref={endRef} />
            </div>
          </div>

          <div className="border-t border-border px-3 py-2.5">
            {selectionText && (
              <div className="mb-2 flex items-center justify-between gap-2 rounded-lg border border-dashed border-accent px-2.5 py-2 text-xs text-fg-dim">
                <span className="truncate">
                  已选中 {selectionText.length} 字符
                </span>
                <button
                  className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
                  disabled={busy || !pdfReady}
                  onClick={() => void send('', 'selection')}
                >
                  解释选中内容
                </button>
              </div>
            )}

            {extracting && (
              <p className="m-0 text-xs text-fg-dim">
                正在提取全文… {extracting.done}/{extracting.total} 页
              </p>
            )}

            <textarea
              className="max-h-40 min-h-16 w-full resize-none rounded-lg border border-border bg-bg px-2.5 py-2 text-[13px] text-fg outline-none focus:border-accent disabled:opacity-60"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.key === 'Enter' &&
                  !event.shiftKey &&
                  !event.nativeEvent.isComposing
                ) {
                  event.preventDefault();
                  void send(draft, 'typed');
                }
              }}
              placeholder={
                pdfReady
                  ? scope === 'current_page'
                    ? '基于当前页提问…（Enter 发送）'
                    : '基于全文提问…（Enter 发送）'
                  : 'PDF 加载中，问答暂不可用'
              }
              disabled={!pdfReady || busy}
              maxLength={2000}
            />

            <div className="mt-2 flex items-center justify-between gap-2">
              <span className="text-xs text-fg-dim">
                内容会发送到当前模型服务商
              </span>
              <button
                className="rounded-md border border-accent bg-accent px-4 py-[7px] text-[12.5px] text-white disabled:opacity-45"
                disabled={!pdfReady || busy || !draft.trim()}
                onClick={() => void send(draft, 'typed')}
              >
                {busy ? '回答中…' : '发送'}
              </button>
            </div>
          </div>
        </>
      )}
    </aside>
  );
}
