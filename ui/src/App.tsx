import { useCallback, useEffect, useState } from 'react';

import {
  getLlmSettings,
  health,
  isTauri,
  saveLlmSettings,
  searchPapers,
  summarizePaper,
} from './api/ipc';
import type { LlmProviderConfig, LlmSettings, Paper, Summary } from './types';

type Conn = 'checking' | 'ok' | 'error';

export default function App() {
  const [conn, setConn] = useState<Conn>('checking');
  const [connMsg, setConnMsg] = useState('');

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsSaved, setSettingsSaved] = useState(false);

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
