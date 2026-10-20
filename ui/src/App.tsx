import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  browseFeed,
  getLlmSettings,
  health,
  isTauri,
  saveLlmSettings,
  searchPapers,
  summarizePaper,
} from './api/ipc';
import type { LlmProviderConfig, LlmSettings, Paper, Summary } from './types';

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
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsSaved, setSettingsSaved] = useState(false);

  const [view, setView] = useState<View>('feed');
  const [interests, setInterests] = useState<string[]>(() => loadInterests());
  const [interestDraft, setInterestDraft] = useState('');
  const [selectedInterest, setSelectedInterest] = useState<string | null>(null);

  const [query, setQuery] = useState('');
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
        setSettings(loaded);
        setSelectedProviderId(loaded.active_id || loaded.providers[0]?.id || null);
      })
      .catch(() => {
        // The settings page retries when opened.
      });
  }, []);

  const openSettings = useCallback(async () => {
    setSettingsOpen(true);
    setSettingsError(null);
    setSettingsSaved(false);

    if (!settings) {
      try {
        const loaded = await getLlmSettings();
        setSettings(loaded);
        setSelectedProviderId(loaded.active_id || loaded.providers[0]?.id || null);
      } catch (e: unknown) {
        setSettingsError(String(e));
      }
    } else if (!selectedProviderId) {
      setSelectedProviderId(settings.active_id || settings.providers[0]?.id || null);
    }
  }, [settings, selectedProviderId]);

  const selectedProvider =
    settings?.providers.find((provider) => provider.id === selectedProviderId) ?? null;
  const activeProvider =
    settings?.providers.find((provider) => provider.id === settings.active_id) ?? null;

  const updateProvider = useCallback(
    (field: keyof Omit<LlmProviderConfig, 'id'>, value: string) => {
      setSettings((current) => {
        if (!current) return current;
        return {
          ...current,
          providers: current.providers.map((provider) =>
            provider.id === selectedProviderId ? { ...provider, [field]: value } : provider,
          ),
        };
      });
      setSettingsSaved(false);
    },
    [selectedProviderId],
  );

  const addProvider = useCallback(() => {
    const id = `provider-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const provider: LlmProviderConfig = {
      id,
      name: '新服务商',
      api_key: '',
      base_url: 'https://api.deepseek.com',
      model: 'deepseek-chat',
    };

    setSettings((current) =>
      current
        ? {
            ...current,
            providers: [...current.providers, provider],
            active_id: current.active_id || id,
          }
        : current,
    );
    setSelectedProviderId(id);
    setSettingsSaved(false);
  }, []);

  const removeProvider = useCallback(
    (id: string) => {
      if (!settings || settings.providers.length <= 1) return;

      const providers = settings.providers.filter((provider) => provider.id !== id);
      const nextProvider = providers[0];
      if (!nextProvider) return;

      const active_id =
        settings.active_id === id ? nextProvider.id : settings.active_id;

      setSettings({ ...settings, providers, active_id });
      setSelectedProviderId(nextProvider.id);
      setSettingsSaved(false);
    },
    [settings],
  );

  const setActiveProvider = useCallback(() => {
    if (!selectedProviderId) return;

    setSettings((current) =>
      current ? { ...current, active_id: selectedProviderId } : current,
    );
    setSettingsSaved(false);
  }, [selectedProviderId]);

  const applyPreset = useCallback(
    (preset: 'deepseek' | 'openai') => {
      const values =
        preset === 'deepseek'
          ? {
              name: 'DeepSeek 官方',
              base_url: 'https://api.deepseek.com',
              model: 'deepseek-chat',
            }
          : {
              name: 'OpenAI',
              base_url: 'https://api.openai.com/v1',
              model: 'gpt-4o-mini',
            };

      setSettings((current) => {
        if (!current) return current;
        return {
          ...current,
          providers: current.providers.map((provider) =>
            provider.id === selectedProviderId ? { ...provider, ...values } : provider,
          ),
        };
      });
      setSettingsSaved(false);
    },
    [selectedProviderId],
  );

  const submitSettings = useCallback(async () => {
    if (!settings || settingsBusy) return;

    setSettingsBusy(true);
    setSettingsError(null);
    setSettingsSaved(false);

    try {
      const saved = await saveLlmSettings(settings);
      setSettings(saved);
      setSelectedProviderId(saved.active_id || saved.providers[0]?.id || null);
      setSettingsSaved(true);
    } catch (e: unknown) {
      setSettingsError(String(e));
    } finally {
      setSettingsBusy(false);
    }
  }, [settings, settingsBusy]);

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
    try {
      setPapers(await searchPapers(q, 10));
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
            onClick={() => void openSettings()}
            title="当前模型服务"
          >
            {activeProvider?.name || '未配置模型'}
          </button>
          <button className="settings-btn" onClick={() => void openSettings()}>
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
            <button className="sidebar-item" onClick={() => void openSettings()}>
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

      {settingsOpen && (
        <div className="settings-backdrop" onClick={() => setSettingsOpen(false)}>
          <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
            <header className="settings-header">
              <div>
                <h2>设置</h2>
                <p className="hint">
                  配置多个模型服务商，随时切换当前使用的 API。支持任意 OpenAI 兼容端点。
                </p>
              </div>
              <button
                className="icon-btn"
                onClick={() => setSettingsOpen(false)}
                aria-label="关闭"
              >
                ✕
              </button>
            </header>

            <div className="settings-body">
              <aside className="provider-list">
                <div className="provider-list-title">模型服务</div>

                {settings?.providers.map((provider) => (
                  <button
                    key={provider.id}
                    className={`provider-row ${
                      provider.id === selectedProviderId ? 'provider-row--selected' : ''
                    }`}
                    onClick={() => setSelectedProviderId(provider.id)}
                  >
                    <span className="provider-avatar">
                      {provider.name.trim().slice(0, 1) || '?'}
                    </span>
                    <span className="provider-meta">
                      <span className="provider-name">
                        {provider.name || '未命名服务商'}
                      </span>
                      <span className="provider-model">
                        {provider.model || '未设置模型'}
                      </span>
                    </span>
                    {provider.id === settings?.active_id && (
                      <span className="provider-badge">当前</span>
                    )}
                  </button>
                ))}

                <button className="provider-add" onClick={addProvider}>
                  ＋ 新增服务商
                </button>
              </aside>

              <section className="provider-editor">
                {!settings && !settingsError && <p className="empty">加载中…</p>}
                {settingsError && <div className="error">{settingsError}</div>}

                {settings && !selectedProvider && (
                  <p className="empty">选择或新增一个服务商。</p>
                )}

                {settings && selectedProvider && (
                  <>
                    <div className="preset-row">
                      <span className="preset-label">预设</span>
                      <button onClick={() => applyPreset('deepseek')}>DeepSeek</button>
                      <button onClick={() => applyPreset('openai')}>OpenAI</button>
                      <span className="preset-hint">填充 Base URL 和模型，API Key 需自行获取</span>
                    </div>

                    <label className="field">
                      <span>名称</span>
                      <input
                        value={selectedProvider.name}
                        onChange={(e) => updateProvider('name', e.target.value)}
                        placeholder="例如：DeepSeek 官方"
                        spellCheck={false}
                      />
                    </label>

                    <label className="field">
                      <span>Base URL</span>
                      <input
                        value={selectedProvider.base_url}
                        onChange={(e) => updateProvider('base_url', e.target.value)}
                        placeholder="https://api.deepseek.com"
                        spellCheck={false}
                      />
                    </label>

                    <label className="field">
                      <span>API Key</span>
                      <input
                        type="password"
                        value={selectedProvider.api_key}
                        onChange={(e) => updateProvider('api_key', e.target.value)}
                        placeholder="sk-..."
                        spellCheck={false}
                      />
                    </label>

                    <label className="field">
                      <span>模型</span>
                      <input
                        value={selectedProvider.model}
                        onChange={(e) => updateProvider('model', e.target.value)}
                        placeholder="deepseek-chat"
                        spellCheck={false}
                      />
                    </label>

                    <div className="provider-actions">
                      <button
                        className="ghost"
                        onClick={setActiveProvider}
                        disabled={selectedProvider.id === settings.active_id}
                      >
                        {selectedProvider.id === settings.active_id
                          ? '当前使用中'
                          : '设为当前'}
                      </button>
                      <button
                        className="danger"
                        onClick={() => removeProvider(selectedProvider.id)}
                        disabled={settings.providers.length <= 1}
                      >
                        删除
                      </button>
                    </div>
                  </>
                )}
              </section>
            </div>

            <footer className="settings-footer">
              <span className="hint">
                {settingsSaved
                  ? '已保存'
                  : 'API key 只保存在本机配置文件中，不会写入日志。'}
              </span>
              <div className="settings-footer-actions">
                <button className="ghost" onClick={() => setSettingsOpen(false)}>
                  关闭
                </button>
                <button
                  onClick={() => void submitSettings()}
                  disabled={settingsBusy || !settings || settings.providers.length === 0}
                >
                  {settingsBusy ? '保存中' : '保存'}
                </button>
              </div>
            </footer>
          </div>
        </div>
      )}
    </div>
  );
}
