import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  browseFeed,
  getLlmSettings,
  health,
  isTauri,
  searchPapers,
  summarizePaperStream,
} from './api/ipc';
import ReaderView from './components/ReaderView';
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
  const [summaryStream, setSummaryStream] = useState('');
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const [readerOpen, setReaderOpen] = useState(false);
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
    setSummaryStream('');
    setSummaryError(null);
    setReaderOpen(false);

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
    setSummaryStream('');
    setSummaryError(null);
    setReaderOpen(false);
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
    setSummaryStream('');
    setSummaryBusy(true);
    setSummaryError(null);
    setReaderOpen(false);

    try {
      const result = await summarizePaperStream(paper, (delta) => {
        if (requestId === summaryRequestId.current) {
          setSummaryStream((current) => current + delta);
        }
      });
      if (requestId === summaryRequestId.current) setSummary(result);
    } catch (e: unknown) {
      if (requestId === summaryRequestId.current) setSummaryError(String(e));
    } finally {
      if (requestId === summaryRequestId.current) setSummaryBusy(false);
    }
  }, []);

  const renderCard = (paper: Paper) => (
    <button
      key={paper.id}
      className={`mb-1.5 block w-full rounded-[10px] border px-3.5 py-3 text-left text-inherit transition-colors hover:bg-bg-hover ${
        selected?.id === paper.id
          ? 'border-accent bg-bg-hover'
          : 'border-transparent'
      }`}
      onClick={() => void open(paper)}
    >
      <h3 className="mb-1 text-sm font-semibold leading-snug">{paper.title}</h3>
      <p className="m-0 text-xs text-fg-dim">
        {paper.authors.slice(0, 3).join(', ')}
        {paper.authors.length > 3 && ' 等'}
        {' · '}
        {paper.published.slice(0, 10)}
        {' · '}
        {paper.source}
      </p>
      <p className="mt-1.5 line-clamp-2 text-[12.5px] leading-relaxed text-fg-dim">
        {paper.abstract_text}
      </p>
      {paper.categories.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {paper.categories.slice(0, 3).map((category) => (
            <span
              key={category}
              className="rounded-full border border-border px-[7px] py-px text-[10.5px] text-fg-dim"
            >
              {category}
            </span>
          ))}
        </div>
      )}
    </button>
  );

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between gap-3 border-b border-border bg-bg-raised px-4 py-2.5 [-webkit-app-region:drag]">
        <span className="font-semibold tracking-[0.2px]">PaperFlow</span>
        <div className="flex items-center gap-3 [-webkit-app-region:no-drag]">
          <span
            className={`text-xs ${
              conn === 'ok'
                ? 'text-ok'
                : conn === 'error'
                  ? 'text-danger'
                  : 'text-fg-dim'
            }`}
          >
            {connMsg || '检查中…'}
          </span>
          <button
            className="max-w-40 truncate rounded-full border border-border px-2.5 py-1 text-[11.5px] text-fg-dim hover:border-accent hover:text-accent"
            onClick={() => setSettingsOpen(true)}
            title="当前模型服务"
          >
            {activeProviderName || '未配置模型'}
          </button>
          <button
            className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:bg-bg-hover hover:text-fg"
            onClick={() => setSettingsOpen(true)}
          >
            设置
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="flex w-50 flex-none flex-col gap-3 overflow-y-auto border-r border-border bg-bg-raised px-2.5 py-3 max-[960px]:w-44">
          <nav className="flex flex-col gap-0.5">
            <button
              className={`flex w-full items-center gap-2 overflow-hidden rounded-lg border px-2.5 py-2 text-left text-[13px] transition-colors ${
                view === 'feed'
                  ? 'border-accent bg-bg-hover font-medium text-fg'
                  : 'border-transparent text-fg-dim hover:bg-bg-hover hover:text-fg'
              }`}
              onClick={() => setView('feed')}
            >
              <span className="w-4 text-center opacity-80">▤</span>
              推荐
            </button>
            <button
              className={`flex w-full items-center gap-2 overflow-hidden rounded-lg border px-2.5 py-2 text-left text-[13px] transition-colors ${
                view === 'search'
                  ? 'border-accent bg-bg-hover font-medium text-fg'
                  : 'border-transparent text-fg-dim hover:bg-bg-hover hover:text-fg'
              }`}
              onClick={() => setView('search')}
            >
              <span className="w-4 text-center opacity-80">⌕</span>
              搜索
            </button>
          </nav>

          <div className="flex flex-col gap-0.5">
            <div className="px-2.5 pb-1.5 pt-1 text-[11px] uppercase tracking-[0.6px] text-fg-dim">
              兴趣
            </div>
            <button
              className={`flex w-full items-center gap-2 overflow-hidden rounded-lg border px-2.5 py-2 text-left text-[13px] transition-colors ${
                view === 'feed' && selectedInterest === null
                  ? 'border-accent bg-bg-hover font-medium text-fg'
                  : 'border-transparent text-fg-dim hover:bg-bg-hover hover:text-fg'
              }`}
              onClick={() => {
                setView('feed');
                setSelectedInterest(null);
              }}
            >
              <span className="w-4 text-center opacity-80">✦</span>
              全部兴趣
              <span className="ml-auto text-[11px] text-fg-dim">
                {interests.length}
              </span>
            </button>

            {interests.map((interest) => (
              <div key={interest} className="group flex items-center gap-0.5">
                <button
                  className={`min-w-0 flex-1 truncate rounded-lg border px-2.5 py-2 text-left text-[12.5px] ${
                    view === 'feed' && selectedInterest === interest
                      ? 'border-transparent bg-bg-hover text-accent'
                      : 'border-transparent text-fg-dim hover:bg-bg-hover hover:text-fg'
                  }`}
                  onClick={() => {
                    setView('feed');
                    setSelectedInterest(interest);
                  }}
                  title={interest}
                >
                  {interest}
                </button>
                <button
                  className="h-[22px] w-[22px] flex-none rounded-md text-[13px] text-fg-dim opacity-0 transition-opacity hover:bg-[rgba(255,122,122,0.12)] hover:text-danger focus-visible:opacity-100 group-hover:opacity-100"
                  onClick={() => removeInterest(interest)}
                  aria-label={`删除兴趣 ${interest}`}
                >
                  ×
                </button>
              </div>
            ))}

            <form
              className="mt-1.5 flex gap-1"
              onSubmit={(e) => {
                e.preventDefault();
                addInterest();
              }}
            >
              <input
                className="min-w-0 flex-1 rounded-md border border-border bg-bg px-2 py-1.5 text-xs text-fg outline-none focus:border-accent"
                value={interestDraft}
                onChange={(e) => setInterestDraft(e.target.value)}
                placeholder="添加英文兴趣…"
                spellCheck={false}
                maxLength={80}
              />
              <button
                type="submit"
                className="w-7 flex-none rounded-md border border-border text-fg-dim hover:border-accent hover:text-accent disabled:opacity-40"
                disabled={!interestDraft.trim() || interests.length >= MAX_INTERESTS}
              >
                ＋
              </button>
            </form>
            {interests.length >= MAX_INTERESTS && (
              <p className="mx-2.5 mt-0.5 text-[11px] text-fg-dim">
                最多 {MAX_INTERESTS} 个兴趣
              </p>
            )}
          </div>

          <div className="mt-auto border-t border-border pt-2">
            <button
              className="flex w-full items-center gap-2 overflow-hidden rounded-lg border border-transparent px-2.5 py-2 text-left text-[13px] text-fg-dim hover:bg-bg-hover hover:text-fg"
              onClick={() => setSettingsOpen(true)}
            >
              <span className="w-4 text-center opacity-80">⚙</span>
              设置
            </button>
          </div>
        </aside>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          {view === 'search' ? (
            <div className="flex gap-2 border-b border-border px-4 py-3">
              <input
                className="flex-1 rounded-lg border border-border bg-bg-raised px-3 py-2 text-fg outline-none focus:border-accent"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void runSearch();
                }}
                placeholder="搜索论文…（例如：diffusion models）"
                spellCheck={false}
              />
              <button
                className="rounded-lg border border-accent bg-accent px-4 py-2 font-medium text-white disabled:opacity-45"
                onClick={() => void runSearch()}
                disabled={searchBusy || !query.trim()}
              >
                {searchBusy ? '查询中' : '搜索'}
              </button>
            </div>
          ) : (
            <header className="flex items-start justify-between gap-3 border-b border-border px-4 py-3">
              <div>
                <h2 className="m-0 text-base">
                  {selectedInterest ?? '全部兴趣'}
                </h2>
                <p className="m-0 mt-0.5 text-xs text-fg-dim">
                  {activeInterests.length === 0
                    ? '先在左侧添加兴趣'
                    : `按最新发表排序 · ${activeInterests.length} 个兴趣`}
                </p>
              </div>
              <button
                className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
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
              <p className="mx-4 mt-2 text-xs text-fg-dim">
                已按英文关键词搜索：
                <strong className="font-medium text-accent">{effectiveQuery}</strong>
              </p>
            )}
          {view === 'search' &&
            effectiveQuery &&
            effectiveQuery === query.trim() &&
            containsCjk(effectiveQuery) && (
              <p className="mx-4 mt-2 text-xs text-fg-dim">
                未配置 LLM，已按中文原文检索，结果可能较少。
              </p>
            )}

          {error && (
            <div className="mx-4 mb-2 rounded-lg border border-danger px-3.5 py-2.5 text-[13px] text-danger">
              {error}
            </div>
          )}

          <main className="grid min-h-0 flex-1 grid-cols-[minmax(280px,40%)_1fr] max-[960px]:grid-cols-[minmax(260px,46%)_1fr]">
            <section className="overflow-y-auto border-r border-border p-2">
              {view === 'search' ? (
                <>
                  {papers.length === 0 && !searchBusy && (
                    <p className="px-4 py-6 text-[13px] text-fg-dim">
                      还没有结果。输入关键词回车。
                    </p>
                  )}
                  {papers.map(renderCard)}
                </>
              ) : (
                <>
                  {feedError && (
                    <div className="mx-2 mb-2 flex items-center justify-between gap-2 rounded-lg border border-danger px-3 py-2 text-[12.5px] text-danger">
                      <span>{feedError}</span>
                      <button
                        className="rounded-md border border-danger px-3 py-1 text-xs text-danger hover:bg-[rgba(255,122,122,0.12)]"
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
                    <p className="px-4 py-6 text-[13px] text-fg-dim">
                      {activeInterests.length === 0
                        ? '还没有兴趣。在左侧添加一个主题。'
                        : '最近没有匹配的论文。'}
                    </p>
                  )}
                  {feedPapers.length === 0 && feedLoading && (
                    <p className="px-4 py-6 text-[13px] text-fg-dim">
                      正在加载推荐…
                    </p>
                  )}

                  {feedPapers.map(renderCard)}

                  <div ref={feedSentinelRef} className="h-px" />

                  {feedLoading && feedPapers.length > 0 && (
                    <p className="px-4 py-6 text-[13px] text-fg-dim">
                      加载更多…
                    </p>
                  )}
                  {!feedLoading && feedHasMore && feedPapers.length > 0 && (
                    <button
                      className="my-2 block w-[calc(100%-16px)] rounded-lg border border-dashed border-border p-2 text-fg-dim hover:border-accent hover:text-accent"
                      onClick={loadMore}
                    >
                      加载更多
                    </button>
                  )}
                  {!feedHasMore && feedPapers.length > 0 && (
                    <p className="p-3 text-center text-xs text-fg-dim">
                      没有更多了
                    </p>
                  )}
                </>
              )}
            </section>

            <section className="overflow-y-auto px-6 py-5 max-[960px]:p-4">
              {!selected && (
                <p className="px-4 py-6 text-[13px] text-fg-dim">
                  选一篇论文查看分析。
                </p>
              )}
              {selected && (
                <>
                  <h2 className="mb-1.5 text-xl leading-snug">
                    {selected.title}
                  </h2>
                  <p className="m-0 text-xs text-fg-dim">
                    {selected.authors.join(', ')}
                    {selected.venue && ` · ${selected.venue}`}
                    {` · 来源 ${selected.source}`}
                  </p>
                  <div className="mt-2.5 flex flex-wrap gap-1.5">
                    {selected.categories.map((c) => (
                      <span
                        key={c}
                        className="rounded-full border border-border px-2 py-0.5 text-[11.5px] text-fg-dim"
                      >
                        {c}
                      </span>
                    ))}
                  </div>

                  <button
                    className="mt-3 rounded-lg border border-accent bg-accent px-[18px] py-2 text-white disabled:opacity-45"
                    onClick={() => setReaderOpen(true)}
                    disabled={!selected.pdf_url}
                  >
                    开始阅读
                  </button>
                  {!selected.pdf_url && (
                    <p className="m-0 text-xs text-fg-dim">
                      该论文没有可用的 PDF 链接。
                    </p>
                  )}

                  <h4 className="mb-2 mt-5 text-xs font-semibold uppercase tracking-[0.6px] text-fg-dim">
                    摘要
                  </h4>
                  <p className="mb-2.5 whitespace-pre-wrap">
                    {selected.abstract_text}
                  </p>

                  <h4 className="mb-2 mt-5 text-xs font-semibold uppercase tracking-[0.6px] text-fg-dim">
                    LLM 分析
                  </h4>
                  {summaryBusy && !summaryStream && (
                    <p className="px-4 py-6 text-[13px] text-fg-dim">
                      分析中…
                    </p>
                  )}
                  {summaryBusy && summaryStream && (
                    <p className="mb-2.5 whitespace-pre-wrap">{summaryStream}</p>
                  )}
                  {!summaryBusy && !summary && summaryStream && (
                    <p className="mb-2.5 whitespace-pre-wrap">{summaryStream}</p>
                  )}
                  {summaryError && (
                    <div className="my-2 flex items-center justify-between gap-2.5 rounded-lg border border-danger px-3 py-2 text-[12.5px] text-danger">
                      <span>{summaryError}</span>
                      <button
                        className="rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent"
                        onClick={() => void open(selected)}
                      >
                        重试
                      </button>
                    </div>
                  )}
                  {summary && (
                    <>
                      <p className="mb-2.5 whitespace-pre-wrap">
                        {summary.short}
                      </p>
                      <p className="mb-2.5 whitespace-pre-wrap">
                        {summary.detailed}
                      </p>
                      <ul className="m-0 pl-5">
                        {summary.key_points.map((k, i) => (
                          <li key={i} className="mb-1">
                            {k}
                          </li>
                        ))}
                      </ul>
                      <p className="m-0 text-xs text-fg-dim">
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

      {readerOpen && selected && (
        <ReaderView
          key={selected.id}
          paper={selected}
          summary={summary}
          summaryStream={summaryStream}
          summaryBusy={summaryBusy}
          summaryError={summaryError}
          onRetrySummary={() => void open(selected)}
          onClose={() => setReaderOpen(false)}
        />
      )}
    </div>
  );
}
