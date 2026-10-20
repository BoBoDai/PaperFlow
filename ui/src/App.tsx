import { useCallback, useEffect, useState } from 'react';

import { health, isTauri, searchPapers, summarizePaper } from './api/ipc';
import type { Paper, Summary } from './types';

type Conn = 'checking' | 'ok' | 'error';

export default function App() {
  const [conn, setConn] = useState<Conn>('checking');
  const [connMsg, setConnMsg] = useState('');

  const [query, setQuery] = useState('');
  const [papers, setPapers] = useState<Paper[]>([]);
  const [selected, setSelected] = useState<Paper | null>(null);
  const [summary, setSummary] = useState<Summary | null>(null);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 启动时确认 IPC 链路
  useEffect(() => {
    if (!isTauri()) {
      setConn('error');
      setConnMsg('未在 Tauri 中运行 —— 请用 pnpm app:dev 启动');
      return;
    }
    health()
      .then((msg) => {
        setConn('ok');
        setConnMsg(msg);
      })
      .catch((e: unknown) => {
        setConn('error');
        setConnMsg(String(e));
      });
  }, []);

  const runSearch = useCallback(async () => {
    const q = query.trim();
    if (!q || busy) return;
    setBusy(true);
    setError(null);
    setSelected(null);
    setSummary(null);
    try {
      setPapers(await searchPapers(q, 10));
    } catch (e: unknown) {
      setError(String(e));
      setPapers([]);
    } finally {
      setBusy(false);
    }
  }, [query, busy]);

  const open = useCallback(async (paper: Paper) => {
    setSelected(paper);
    setSummary(null);
    setBusy(true);
    setError(null);
    try {
      setSummary(await summarizePaper(paper));
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">PaperFlow</span>
        <span className={`conn conn--${conn}`}>{connMsg || '检查中…'}</span>
      </header>

      <div className="searchbar">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void runSearch();
          }}
          placeholder="搜索论文…（例如：diffusion models）"
          spellCheck={false}
        />
        <button onClick={() => void runSearch()} disabled={busy || !query.trim()}>
          {busy ? '查询中' : '搜索'}
        </button>
      </div>

      {error && <div className="error">{error}</div>}

      <main className="panes">
        <section className="list">
          {papers.length === 0 && !busy && (
            <p className="empty">还没有结果。输入关键词回车。</p>
          )}
          {papers.map((p) => (
            <button
              key={p.id}
              className={`card ${selected?.id === p.id ? 'card--active' : ''}`}
              onClick={() => void open(p)}
            >
              <h3>{p.title}</h3>
              <p className="meta">
                {p.authors.slice(0, 3).join(', ')}
                {p.authors.length > 3 && ' 等'}
                {' · '}
                {p.published.slice(0, 10)}
              </p>
              <p className="abstract">{p.abstract_text}</p>
            </button>
          ))}
        </section>

        <section className="detail">
          {!selected && <p className="empty">选一篇论文查看分析。</p>}
          {selected && (
            <>
              <h2>{selected.title}</h2>
              <p className="meta">
                {selected.authors.join(', ')}
                {selected.venue && ` · ${selected.venue}`}
                {` · 来源 ${selected.source}`}
              </p>
              <div className="tags">
                {selected.categories.map((c) => (
                  <span key={c} className="tag">
                    {c}
                  </span>
                ))}
              </div>

              <h4>摘要</h4>
              <p className="body">{selected.abstract_text}</p>

              <h4>LLM 分析</h4>
              {busy && <p className="empty">分析中…</p>}
              {summary && (
                <>
                  <p className="body">{summary.short}</p>
                  <p className="body">{summary.detailed}</p>
                  <ul className="points">
                    {summary.key_points.map((k, i) => (
                      <li key={i}>{k}</li>
                    ))}
                  </ul>
                  <p className="meta">
                    模型 {summary.model} · prompt {summary.prompt_version}
                  </p>
                </>
              )}
            </>
          )}
        </section>
      </main>
    </div>
  );
}
