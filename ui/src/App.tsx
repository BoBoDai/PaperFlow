import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  browseFeed,
  getLlmSettings,
  health,
  isTauri,
  searchPapers,
  summarizePaper,
} from './api/ipc';
import SettingsPanel from './components/SettingsPanel';
import type { LlmSettingsView, Paper, Summary } from './types';

type Conn = 'checking' | 'ok' | 'error';
type View = 'feed' | 'search';

const FEED_PAGE_SIZE = 12;
const MAX_INTERESTS = 8;
const INTERESTS_KEY = 'paperflow.interests.v1';

const DEFAULT_INTERESTS = [
  'large language models',
  'diffusion models',
  'computer vision',
  'reinforcement learning',
];

function loadInterests(): string[] {
  try {
    const raw = localStorage.getItem(INTERESTS_KEY);
    if (raw === null) return [...DEFAULT_INTERESTS];

    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [...DEFAULT_INTERESTS];

    const seen = new Set<string>();
    return parsed
      .filter((item): item is string => typeof item === 'string')
      .map((item) => item.trim())
      .filter((item) => item.length > 0)
      .filter((item) => {
        const key = item.toLowerCase();
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .slice(0, MAX_INTERESTS);
  } catch {
    return [...DEFAULT_INTERESTS];
  }
}

function saveInterests(interests: string[]): void {
  try {
    localStorage.setItem(INTERESTS_KEY, JSON.stringify(interests));
  } catch {
    // Storage can be unavailable; the in-memory list still works.
  }
}

function containsCjk(text: string): boolean {
  return /[㐀-䶿一-鿿豈-﫿]/.test(text);
}

function titleKey(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, '')
    .split(/\s+/)
    .filter(Boolean)
    .join(' ');
}

function mergeUniquePapers(existing: Paper[], incoming: Paper[]): Paper[] {
  const ids = new Set(existing.map((paper) => paper.id));
  const titles = new Set(existing.map((paper) => titleKey(paper.title)));
  const merged = [...existing];

  for (const paper of incoming) {
    const key = titleKey(paper.title);
    if (ids.has(paper.id) || titles.has(key)) continue;
    ids.add(paper.id);
    titles.add(key);
    merged.push(paper);
  }

  return merged.sort(
    (a, b) =>
      Date.parse(b.published) - Date.parse(a.published) ||
      a.id.localeCompare(b.id),
  );
}

export default function App() {
  const [conn, setConn] = useState<Conn>('checking');
  const [connMsg, setConnMsg] = useState('');

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeProviderName, setActiveProviderName] = useState<string | null>(
    null,
  );

  const [view, setView] = useState<View>('feed');
  const [interests, setInterests] = useState<string[]>(() => loadInterests());
  const [interestDraft, setInterestDraft] = useState('');
  const [selectedInterest, setSelectedInterest] = useState<string | null>(null);

  const [query, setQuery] = useState('');
  const [effectiveQuery, setEffectiveQuery] = useState<string | null>(null);
  const [papers, setPapers] = useState<Paper[]>([]);
  const [selected, setSelected] = useState<Paper | null>(null);
  const [summary, setSummary] = useState<Summary | null>(null);

  const [searchBusy, setSearchBusy] = useState(false);
  const [summaryBusy, setSummaryBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [feedPapers, setFeedPapers] = useState<Paper[]>([]);
  const [feedOffset, setFeedOffset] = useState(0);
  const [feedHasMore, setFeedHasMore] = useState(true);
  const [feedLoading, setFeedLoading] = useState(false);
  const [feedError, setFeedError] = useState<string | null>(null);

  const feedRequestId = useRef(0);
  const summaryRequestId = useRef(0);
  const feedLoadingRef = useRef(false);
  const feedSentinelRef = useRef<HTMLDivElement | null>(null);
  const lastFeedKey = useRef('');

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

    getLlmSettings()
      .then((loaded) => {
        const active = loaded.providers.find(
          (provider) => provider.id === loaded.active_id,
        );
        setActiveProviderName(active?.name ?? null);
      })
      .catch(() => {
        // The settings panel retries when opened.
      });
  }, []);

  const activeInterests = useMemo(
    () => (selectedInterest === null ? interests : [selectedInterest]),
    [selectedInterest, interests],
  );

  const fetchFeedPage = useCallback(
    async (channelInterests: string[], offset: number, replace: boolean) => {
      if (channelInterests.length === 0) {
        setFeedPapers([]);
        setFeedOffset(0);
        setFeedHasMore(false);
        setFeedError(null);
        return;
      }

      if (!replace && feedLoadingRef.current) return;

      const requestId = ++feedRequestId.current;
      feedLoadingRef.current = true;
      setFeedLoading(true);
      setFeedError(null);

      try {
        const page = await browseFeed(channelInterests, offset, FEED_PAGE_SIZE);
        if (requestId !== feedRequestId.current) return;

        setFeedPapers((current) =>
          mergeUniquePapers(replace ? [] : current, page.papers),
        );
        setFeedOffset(offset + FEED_PAGE_SIZE);
        setFeedHasMore(page.has_more);
      } catch (e: unknown) {
        if (requestId !== feedRequestId.current) return;
        setFeedError(String(e));
        if (replace) {
          setFeedPapers([]);
          setFeedOffset(0);
          setFeedHasMore(false);
        }
      } finally {
        if (requestId === feedRequestId.current) {
          feedLoadingRef.current = false;
          setFeedLoading(false);
        }
      }
    },
    [],
  );

  const feedKey = `${view}|${
    selectedInterest ?? `all:${JSON.stringify(interests)}`
  }`;

  useEffect(() => {
    if (view !== 'feed') return;
    if (lastFeedKey.current === feedKey) return;
    lastFeedKey.current = feedKey;

    feedRequestId.current += 1;
    setFeedPapers([]);
    setFeedOffset(0);
    setFeedHasMore(true);
    setFeedError(null);
    setError(null);
    setSelected(null);
    setSummary(null);

    void fetchFeedPage(activeInterests, 0, true);
  }, [view, feedKey, activeInterests, fetchFeedPage]);

  const loadMore = useCallback(() => {
    if (view !== 'feed' || feedLoading || !feedHasMore) return;
    void fetchFeedPage(activeInterests, feedOffset, false);
  }, [view, feedLoading, feedHasMore, activeInterests, feedOffset, fetchFeedPage]);

  useEffect(() => {
    if (view !== 'feed') return;

    const sentinel = feedSentinelRef.current;
    if (!sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          loadMore();
        }
      },
      { rootMargin: '240px' },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [view, loadMore, feedPapers.length]);

  const addInterest = useCallback(() => {
    const value = interestDraft.trim();
    if (!value) return;

    if (
      interests.some((interest) => interest.toLowerCase() === value.toLowerCase())
    ) {
      setInterestDraft('');
      return;
    }
    if (interests.length >= MAX_INTERESTS) return;

    const next = [...interests, value];
    setInterests(next);
    saveInterests(next);
    setInterestDraft('');
    setSelectedInterest(value);
    setView('feed');
  }, [interestDraft, interests]);

  const removeInterest = useCallback(
    (interest: string) => {
      const next = interests.filter((item) => item !== interest);
      setInterests(next);
      saveInterests(next);
      if (selectedInterest === interest) setSelectedInterest(null);
    },
    [interests, selectedInterest],
  );

  const runSearch = useCallback(async () => {
    const q = query.trim();
    if (!q || searchBusy) return;
    setSearchBusy(true);
    setError(null);
    setSelected(null);
    setSummary(null);
    setEffectiveQuery(null);
    try {
      const response = await searchPapers(q, 10);
      setPapers(response.papers);
      setEffectiveQuery(response.query);
    } catch (e: unknown) {
      setError(String(e));
      setPapers([]);
    } finally {
      setSearchBusy(false);
    }
  }, [query, searchBusy]);

  const open = useCallback(async (paper: Paper) => {
    const requestId = ++summaryRequestId.current;
    setSelected(paper);
    setSummary(null);
    setSummaryBusy(true);
    setError(null);
    try {
      const result = await summarizePaper(paper);
      if (requestId === summaryRequestId.current) setSummary(result);
    } catch (e: unknown) {
      if (requestId === summaryRequestId.current) setError(String(e));
    } finally {
      if (requestId === summaryRequestId.current) setSummaryBusy(false);
    }
  }, []);

  const renderCard = (paper: Paper) => (
    <button
      key={paper.id}
      className={`card ${selected?.id === paper.id ? 'card--active' : ''}`}
      onClick={() => void open(paper)}
    >
      <h3>{paper.title}</h3>
      <p className="meta">
        {paper.authors.slice(0, 3).join(', ')}
        {paper.authors.length > 3 && ' 等'}
        {' · '}
        {paper.published.slice(0, 10)}
        {' · '}
        {paper.source}
      </p>
      <p className="abstract">{paper.abstract_text}</p>
      {paper.categories.length > 0 && (
        <div className="card-tags">
          {paper.categories.slice(0, 3).map((category) => (
            <span key={category} className="tag">
              {category}
            </span>
          ))}
        </div>
      )}
    </button>
  );

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">PaperFlow</span>
        <div className="topbar-right">
          <span className={`conn conn--${conn}`}>{connMsg || '检查中…'}</span>
          <button
            className="provider-chip"
            onClick={() => setSettingsOpen(true)}
            title="当前模型服务"
          >
            {activeProviderName || '未配置模型'}
          </button>
          <button className="settings-btn" onClick={() => setSettingsOpen(true)}>
            设置
          </button>
        </div>
      </header>

      <div className="workspace">
        <aside className="sidebar">
          <nav className="sidebar-nav">
            <button
              className={`sidebar-item ${
                view === 'feed' ? 'sidebar-item--active' : ''
              }`}
              onClick={() => setView('feed')}
            >
              <span className="sidebar-icon">▤</span>
              推荐
            </button>
            <button
              className={`sidebar-item ${
                view === 'search' ? 'sidebar-item--active' : ''
              }`}
              onClick={() => setView('search')}
            >
              <span className="sidebar-icon">⌕</span>
              搜索
            </button>
          </nav>

          <div className="sidebar-section">
            <div className="sidebar-section-title">兴趣</div>
            <button
              className={`sidebar-item ${
                view === 'feed' && selectedInterest === null
                  ? 'sidebar-item--active'
                  : ''
              }`}
              onClick={() => {
                setView('feed');
                setSelectedInterest(null);
              }}
            >
              <span className="sidebar-icon">✦</span>
              全部兴趣
              <span className="sidebar-count">{interests.length}</span>
            </button>

            {interests.map((interest) => (
              <div
                key={interest}
                className={`interest-row ${
                  view === 'feed' && selectedInterest === interest
                    ? 'interest-row--active'
                    : ''
                }`}
              >
                <button
                  className="interest-name"
                  onClick={() => {
                    setView('feed');
                    setSelectedInterest(interest);
                  }}
                  title={interest}
                >
                  {interest}
                </button>
                <button
                  className="interest-remove"
                  onClick={() => removeInterest(interest)}
                  aria-label={`删除兴趣 ${interest}`}
                >
                  ×
                </button>
              </div>
            ))}

            <form
              className="interest-add"
              onSubmit={(e) => {
                e.preventDefault();
                addInterest();
              }}
            >
              <input
                value={interestDraft}
                onChange={(e) => setInterestDraft(e.target.value)}
                placeholder="添加英文兴趣…"
                spellCheck={false}
                maxLength={80}
              />
              <button
                type="submit"
                disabled={!interestDraft.trim() || interests.length >= MAX_INTERESTS}
              >
                ＋
              </button>
            </form>
            {interests.length >= MAX_INTERESTS && (
              <p className="sidebar-hint">最多 {MAX_INTERESTS} 个兴趣</p>
            )}
          </div>

          <div className="sidebar-footer">
            <button className="sidebar-item" onClick={() => setSettingsOpen(true)}>
              <span className="sidebar-icon">⚙</span>
              设置
            </button>
          </div>
        </aside>

        <div className="main">
          {view === 'search' ? (
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
              <button
                onClick={() => void runSearch()}
                disabled={searchBusy || !query.trim()}
              >
                {searchBusy ? '查询中' : '搜索'}
              </button>
            </div>
          ) : (
            <header className="feed-header">
              <div>
                <h2>{selectedInterest ?? '全部兴趣'}</h2>
                <p className="meta">
                  {activeInterests.length === 0
                    ? '先在左侧添加兴趣'
                    : `按最新发表排序 · ${activeInterests.length} 个兴趣`}
                </p>
              </div>
              <button
                className="ghost-btn"
                onClick={() => void fetchFeedPage(activeInterests, 0, true)}
                disabled={feedLoading || activeInterests.length === 0}
              >
                刷新
              </button>
            </header>
          )}

          {view === 'search' &&
            effectiveQuery &&
            effectiveQuery !== query.trim() && (
              <p className="search-hint">
                已按英文关键词搜索：<strong>{effectiveQuery}</strong>
              </p>
            )}
          {view === 'search' &&
            effectiveQuery &&
            effectiveQuery === query.trim() &&
            containsCjk(effectiveQuery) && (
              <p className="search-hint">
                未配置 LLM，已按中文原文检索，结果可能较少。
              </p>
            )}

          {error && <div className="error">{error}</div>}

          <main className="panes">
            <section className="list">
              {view === 'search' ? (
                <>
                  {papers.length === 0 && !searchBusy && (
                    <p className="empty">还没有结果。输入关键词回车。</p>
                  )}
                  {papers.map(renderCard)}
                </>
              ) : (
                <>
                  {feedError && (
                    <div className="feed-error">
                      <span>{feedError}</span>
                      <button
                        className="ghost-btn"
                        onClick={() =>
                          void fetchFeedPage(
                            activeInterests,
                            feedOffset,
                            feedPapers.length === 0,
                          )
                        }
                      >
                        重试
                      </button>
                    </div>
                  )}

                  {feedPapers.length === 0 && !feedLoading && !feedError && (
                    <p className="empty">
                      {activeInterests.length === 0
                        ? '还没有兴趣。在左侧添加一个主题。'
                        : '最近没有匹配的论文。'}
                    </p>
                  )}
                  {feedPapers.length === 0 && feedLoading && (
                    <p className="empty">正在加载推荐…</p>
                  )}

                  {feedPapers.map(renderCard)}

                  <div ref={feedSentinelRef} className="feed-sentinel" />

                  {feedLoading && feedPapers.length > 0 && (
                    <p className="empty">加载更多…</p>
                  )}
                  {!feedLoading && feedHasMore && feedPapers.length > 0 && (
                    <button className="load-more" onClick={loadMore}>
                      加载更多
                    </button>
                  )}
                  {!feedHasMore && feedPapers.length > 0 && (
                    <p className="feed-end">没有更多了</p>
                  )}
                </>
              )}
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
                  {summaryBusy && <p className="empty">分析中…</p>}
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
      </div>

      <SettingsPanel
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onSaved={(savedSettings: LlmSettingsView) => {
          const active = savedSettings.providers.find(
            (provider) => provider.id === savedSettings.active_id,
          );
          setActiveProviderName(active?.name ?? null);
        }}
      />
    </div>
  );
}
