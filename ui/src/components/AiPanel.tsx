import { useEffect, useRef, useState } from 'react';

import { askPaper } from '../api/ipc';
import { ipcErrorMessage } from '../lib/ipcError';
import type { Paper, QaScope, QaTurn, Summary } from '../types';

interface AiPanelProps {
  paper: Paper;
  summary: Summary | null;
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
}

let messageSequence = 0;

function messageId(): string {
  messageSequence += 1;
  return `${Date.now()}-${messageSequence}`;
}

export default function AiPanel({
  paper,
  summary,
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
    setBusy(true);

    try {
      let context: string;
      let qaScope: QaScope;
      let page: number | null = currentPage;

      if (mode === 'selection') {
        qaScope = 'selection';
        context = selectionText;
      } else if (scope === 'current_page') {
        qaScope = 'current_page';
        context = await getPageText(currentPage);
      } else {
        qaScope = 'full_paper';
        page = null;
        context = await getFullText((done, total) =>
          setExtracting({ done, total }),
        );
      }

      if (!context.trim()) {
        throw new Error(
          '没有可用的 PDF 文本。该 PDF 可能是扫描版，或文本尚未提取完成。',
        );
      }

      const history: QaTurn[] = messages
        .filter((message) => !message.error)
        .slice(-6)
        .map(({ role, content }) => ({ role, content }));

      const userMessage: ChatMessage = {
        id: messageId(),
        role: 'user',
        content: question,
        display:
          mode === 'selection'
            ? `解释选中内容：${context.slice(0, 120)}${
                context.length > 120 ? '…' : ''
              }`
            : question,
        scope: qaScope,
        page,
      };

      setMessages((current) => [...current, userMessage]);

      const response = await askPaper({
        paper,
        question,
        scope: qaScope,
        context,
        page,
        history,
      });

      if (requestId !== requestIdRef.current) return;

      setMessages((current) => [
        ...current,
        {
          id: messageId(),
          role: 'assistant',
          content: response.answer,
          scope: response.scope,
          page: response.page,
          contextTruncated: response.context_truncated,
          contextChars: response.context_chars,
          model: response.model,
        },
      ]);

      if (mode === 'selection') onClearSelection();
      setDraft('');
    } catch (error: unknown) {
      if (requestId !== requestIdRef.current) return;

      setMessages((current) => [
        ...current,
        {
          id: messageId(),
          role: 'assistant',
          content: ipcErrorMessage(error),
          scope: 'current_page',
          page: null,
          error: true,
        },
      ]);
    } finally {
      if (requestId === requestIdRef.current) {
        setBusy(false);
        setExtracting(null);
      }
    }
  };

  return (
    <aside className="ai-panel">
      <div className="ai-tabs">
        <button
          className={`ai-tab ${tab === 'chat' ? 'ai-tab--active' : ''}`}
          onClick={() => setTab('chat')}
        >
          问答
        </button>
        <button
          className={`ai-tab ${tab === 'summary' ? 'ai-tab--active' : ''}`}
          onClick={() => setTab('summary')}
        >
          分析
        </button>
      </div>

      {tab === 'summary' ? (
        <div className="ai-body">
          {summaryBusy && <p className="empty">分析中…</p>}
          {!summaryBusy && summaryError && (
            <>
              <div className="error">{summaryError}</div>
              <button className="ghost-btn" onClick={onRetrySummary}>
                重试分析
              </button>
            </>
          )}
          {!summaryBusy && !summaryError && summary && (
            <>
              <p className="body">{summary.short}</p>
              <p className="body">{summary.detailed}</p>
              <ul className="points">
                {summary.key_points.map((point, index) => (
                  <li key={index}>{point}</li>
                ))}
              </ul>
              <p className="meta">
                模型 {summary.model} · prompt {summary.prompt_version}
              </p>
            </>
          )}
          {!summaryBusy && !summaryError && !summary && (
            <p className="empty">尚未生成分析。</p>
          )}
        </div>
      ) : (
        <>
          <div className="ai-body chat-body">
            <div className="chat-controls">
              <div className="scope-switch">
                <button
                  className="ghost-btn"
                  aria-pressed={scope === 'current_page'}
                  onClick={() => setScope('current_page')}
                >
                  当前页
                </button>
                <button
                  className="ghost-btn"
                  aria-pressed={scope === 'full_paper'}
                  onClick={() => setScope('full_paper')}
                >
                  全文
                </button>
              </div>
              <span className="meta">
                {scope === 'current_page'
                  ? `第 ${currentPage} 页`
                  : '全文（超长时截断）'}
              </span>
            </div>

            <div className="chat-list">
              {messages.length === 0 && (
                <p className="empty">
                  基于 PDF 内容提问。选中文字后可以让 AI 解释。
                </p>
              )}

              {messages.map((message) => (
                <div
                  key={message.id}
                  className={`chat-bubble chat-bubble--${message.role} ${
                    message.error ? 'chat-bubble--error' : ''
                  }`}
                >
                  {message.display ?? message.content}
                  {message.contextTruncated && (
                    <div className="chat-note">
                      上下文较长，已省略中间部分（发送 {message.contextChars}{' '}
                      字符）
                    </div>
                  )}
                  {message.model && (
                    <div className="chat-note">模型 {message.model}</div>
                  )}
                </div>
              ))}
              <div ref={endRef} />
            </div>
          </div>

          <div className="chat-composer">
            {selectionText && (
              <div className="selection-card">
                <span>已选中 {selectionText.length} 字符</span>
                <button
                  className="ghost-btn"
                  disabled={busy || !pdfReady}
                  onClick={() => void send('', 'selection')}
                >
                  解释选中内容
                </button>
              </div>
            )}

            {extracting && (
              <p className="meta">
                正在提取全文… {extracting.done}/{extracting.total} 页
              </p>
            )}

            <textarea
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

            <div className="chat-composer-actions">
              <span className="meta">
                内容会发送到当前模型服务商
              </span>
              <button
                className="chat-send"
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
