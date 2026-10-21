import { useCallback, useEffect, useRef, useState } from 'react';

import {
  fetchLlmModels,
  getLlmProviderCatalog,
  getLlmSettings,
  saveLlmSettings,
  testLlmConnection,
} from '../api/ipc';
import { formatLlmError, ipcErrorMessage } from '../lib/ipcError';
import type {
  ConnectionTestResult,
  LlmCompat,
  LlmProbeInput,
  LlmProviderView,
  LlmSettingsInput,
  LlmSettingsView,
  MaxTokensField,
  ProviderProfileId,
  ProviderProfileInfo,
  ThinkingFormat,
  ThinkingMode,
} from '../types';

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
  onSaved: (settings: LlmSettingsView) => void;
}

const inputClass =
  'w-full rounded-md border border-border bg-bg px-2.5 py-[7px] text-fg outline-none focus:border-accent';
const selectClass =
  'rounded-md border border-border bg-bg px-2 py-1 text-xs text-fg outline-none focus:border-accent';
const ghostButtonClass =
  'rounded-md border border-border px-3 py-1 text-xs text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45';
const fieldClass = 'mb-3 block';
const fieldLabelClass = 'mb-1 block text-xs text-fg-dim';

interface ProbeState {
  status: 'idle' | 'loading' | 'ok' | 'error';
  result?: ConnectionTestResult;
  error?: string;
}

function newProviderId(): string {
  return `provider-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function thinkingModeLabel(mode: ThinkingMode): string {
  switch (mode) {
    case 'disabled':
      return '禁用';
    case 'adaptive':
      return '自适应';
    default:
      return '默认（不发送）';
  }
}

export default function SettingsPanel({
  open,
  onClose,
  onSaved,
}: SettingsPanelProps) {
  const [settings, setSettings] = useState<LlmSettingsView | null>(null);
  const [catalog, setCatalog] = useState<ProviderProfileInfo[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [clearKeys, setClearKeys] = useState<Record<string, boolean>>({});
  const [keyVisible, setKeyVisible] = useState(false);

  const [modelsByProvider, setModelsByProvider] = useState<
    Record<string, string[]>
  >({});
  const [modelsLoading, setModelsLoading] = useState<Record<string, boolean>>({});
  const [modelErrors, setModelErrors] = useState<Record<string, string>>({});
  const [probes, setProbes] = useState<Record<string, ProbeState>>({});

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const modelRequests = useRef<Record<string, number>>({});
  const probeRequests = useRef<Record<string, number>>({});

  useEffect(() => {
    if (!open) return;

    let cancelled = false;
    setBusy(true);
    setError(null);
    setSaved(false);

    Promise.all([getLlmSettings(), getLlmProviderCatalog()])
      .then(([loaded, providerCatalog]) => {
        if (cancelled) return;
        setSettings(loaded);
        setCatalog(providerCatalog);
        setSelectedId(loaded.active_id || loaded.providers[0]?.id || null);
        setKeyDrafts({});
        setClearKeys({});
        setModelsByProvider({});
        setModelsLoading({});
        setModelErrors({});
        setProbes({});
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(ipcErrorMessage(loadError));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });

    return () => {
      cancelled = true;
    };
  }, [open]);

  const selected =
    settings?.providers.find((provider) => provider.id === selectedId) ?? null;
  const selectedProfile =
    catalog.find((profile) => profile.id === selected?.provider) ?? null;
  const selectedIndex =
    settings?.providers.findIndex((provider) => provider.id === selectedId) ?? 0;

  const updateProvider = useCallback(
    (updater: (provider: LlmProviderView) => LlmProviderView) => {
      setSettings((current) =>
        current
          ? {
              ...current,
              providers: current.providers.map((provider) =>
                provider.id === selectedId ? updater(provider) : provider,
              ),
            }
          : current,
      );
      setSaved(false);
    },
    [selectedId],
  );

  const clearProbe = useCallback(
    (providerId: string) => {
      setProbes((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
    },
    [],
  );

  const updateField = (
    field: 'name' | 'base_url' | 'model',
    value: string,
  ) => {
    if (!selected) return;
    updateProvider((provider) => ({ ...provider, [field]: value }));

    if (field === 'base_url') {
      setModelsByProvider((current) => {
        const next = { ...current };
        delete next[selected.id];
        return next;
      });
      setModelErrors((current) => {
        const next = { ...current };
        delete next[selected.id];
        return next;
      });
      clearProbe(selected.id);
    }

    if (field === 'model') {
      clearProbe(selected.id);
    }
  };

  const updateCompat = (key: keyof LlmCompat, value: unknown) => {
    if (!selected) return;
    updateProvider((provider) => ({
      ...provider,
      compat: { ...provider.compat, [key]: value },
    }));
    clearProbe(selected.id);
  };

  const applyProfile = (profileId: ProviderProfileId) => {
    const profile = catalog.find((item) => item.id === profileId);
    if (!profile || !selected) return;

    updateProvider((provider) => ({
      ...provider,
      provider: profile.id,
      name: profile.id === 'custom' ? provider.name : profile.label,
      base_url: profile.default_base_url || provider.base_url,
      model: profile.default_model || provider.model,
      compat: {},
    }));

    setModelsByProvider((current) => {
      const next = { ...current };
      delete next[selected.id];
      return next;
    });
    setModelErrors((current) => {
      const next = { ...current };
      delete next[selected.id];
      return next;
    });
    clearProbe(selected.id);
  };

  const addProvider = () => {
    if (!settings) return;

    const provider: LlmProviderView = {
      id: newProviderId(),
      name: '新服务商',
      base_url: '',
      model: '',
      provider: 'custom',
      compat: {},
      has_api_key: false,
    };

    setSettings({
      ...settings,
      providers: [...settings.providers, provider],
    });
    setSelectedId(provider.id);
    setSaved(false);
  };

  const removeProvider = (id: string) => {
    if (!settings || settings.providers.length <= 1) return;

    const providers = settings.providers.filter((provider) => provider.id !== id);
    const nextProvider = providers[0];
    if (!nextProvider) return;

    const activeId =
      settings.active_id === id ? nextProvider.id : settings.active_id;

    setSettings({ ...settings, providers, active_id: activeId });
    setSelectedId(nextProvider.id);
    setSaved(false);
  };

  const setActive = () => {
    if (!settings || !selected) return;
    setSettings({ ...settings, active_id: selected.id });
    setSaved(false);
  };

  const setKeyDraft = (id: string, value: string) => {
    setKeyDrafts((current) => ({ ...current, [id]: value }));
    if (value) {
      setClearKeys((current) => ({ ...current, [id]: false }));
    }
    setSaved(false);
  };

  const toggleClearKey = (id: string) => {
    setClearKeys((current) => ({ ...current, [id]: !current[id] }));
    setKeyDrafts((current) => ({ ...current, [id]: '' }));
    setSaved(false);
  };

  const buildProbeInput = (provider: LlmProviderView): LlmProbeInput => ({
    provider_id: provider.id,
    base_url: provider.base_url,
    model: provider.model,
    provider: provider.provider,
    compat: provider.compat,
    api_key: (keyDrafts[provider.id] ?? '').trim() || null,
  });

  const fetchModels = async (provider: LlmProviderView) => {
    const requestId = (modelRequests.current[provider.id] ?? 0) + 1;
    modelRequests.current[provider.id] = requestId;

    setModelsLoading((current) => ({ ...current, [provider.id]: true }));
    setModelErrors((current) => {
      const next = { ...current };
      delete next[provider.id];
      return next;
    });

    try {
      const result = await fetchLlmModels(buildProbeInput(provider));
      if (modelRequests.current[provider.id] !== requestId) return;
      setModelsByProvider((current) => ({
        ...current,
        [provider.id]: result.models,
      }));
    } catch (fetchError: unknown) {
      if (modelRequests.current[provider.id] !== requestId) return;
      setModelErrors((current) => ({
        ...current,
        [provider.id]: formatLlmError(fetchError),
      }));
    } finally {
      if (modelRequests.current[provider.id] === requestId) {
        setModelsLoading((current) => ({
          ...current,
          [provider.id]: false,
        }));
      }
    }
  };

  const testConnection = async (provider: LlmProviderView) => {
    const requestId = (probeRequests.current[provider.id] ?? 0) + 1;
    probeRequests.current[provider.id] = requestId;

    setProbes((current) => ({
      ...current,
      [provider.id]: { status: 'loading' },
    }));

    try {
      const result = await testLlmConnection(buildProbeInput(provider));
      if (probeRequests.current[provider.id] !== requestId) return;
      setProbes((current) => ({
        ...current,
        [provider.id]: { status: 'ok', result },
      }));
    } catch (probeError: unknown) {
      if (probeRequests.current[provider.id] !== requestId) return;
      setProbes((current) => ({
        ...current,
        [provider.id]: {
          status: 'error',
          error: formatLlmError(probeError),
        },
      }));
    }
  };

  const submit = async () => {
    if (!settings || busy) return;

    setBusy(true);
    setError(null);
    setSaved(false);

    try {
      const input: LlmSettingsInput = {
        active_id: settings.active_id,
        providers: settings.providers.map((provider) => ({
          id: provider.id,
          name: provider.name,
          base_url: provider.base_url,
          model: provider.model,
          provider: provider.provider,
          compat: provider.compat,
          api_key: (keyDrafts[provider.id] ?? '').trim() || null,
          clear_api_key: clearKeys[provider.id] === true,
        })),
      };

      const savedSettings = await saveLlmSettings(input);
      setSettings(savedSettings);
      setSelectedId(
        savedSettings.active_id || savedSettings.providers[0]?.id || null,
      );
      setKeyDrafts({});
      setClearKeys({});
      setSaved(true);
      onSaved(savedSettings);
    } catch (saveError: unknown) {
      setError(ipcErrorMessage(saveError));
    } finally {
      setBusy(false);
    }
  };

  if (!open) return null;

  const keyDraft = selected ? keyDrafts[selected.id] ?? '' : '';
  const models = selected ? modelsByProvider[selected.id] ?? [] : [];
  const probe = selected ? probes[selected.id] : undefined;
  const datalistId = `llm-models-${selectedIndex}`;

  return (
    <div
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/[0.55] p-6"
      onClick={onClose}
    >
      <div
        className="flex h-[min(640px,100%)] w-[min(900px,100%)] flex-col overflow-hidden rounded-[14px] border border-border bg-bg-raised shadow-[0_24px_80px_rgba(0,0,0,0.45)]"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="flex items-start justify-between border-b border-border px-[22px] pb-3.5 pt-[18px]">
          <div>
            <h2 className="m-0 text-[17px]">设置</h2>
            <p className="mt-1 text-xs text-fg-dim">
              配置多个模型服务商。API Key 保存后不再回显；留空表示保持不变。
            </p>
          </div>
          <button
            className="rounded-md px-2 py-1 text-base text-fg-dim hover:bg-bg-hover hover:text-fg"
            onClick={onClose}
            aria-label="关闭"
          >
            ✕
          </button>
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-[240px_1fr]">
          <aside className="flex flex-col gap-1 overflow-y-auto border-r border-border p-3">
            <div className="px-2 pb-2 pt-1 text-[11px] uppercase tracking-[0.6px] text-fg-dim">
              模型服务
            </div>

            {settings?.providers.map((provider) => (
              <button
                key={provider.id}
                className={`flex w-full items-center gap-2.5 rounded-[9px] border px-2.5 py-2.5 text-left text-inherit transition-colors ${
                  provider.id === selectedId
                    ? 'border-accent bg-bg-hover'
                    : 'border-transparent hover:bg-bg-hover'
                }`}
                onClick={() => setSelectedId(provider.id)}
              >
                <span className="grid h-[30px] w-[30px] flex-none place-items-center rounded-lg border border-border bg-bg text-xs font-semibold text-accent">
                  {provider.name.trim().slice(0, 1) || '?'}
                </span>
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[13px] font-medium">
                    {provider.name || '未命名服务商'}
                  </span>
                  <span className="truncate text-[11px] text-fg-dim">
                    {provider.model || '未设置模型'}
                  </span>
                </span>
                <span
                  className={`flex-none text-[10px] ${
                    provider.has_api_key ? 'text-ok' : 'text-fg-dim'
                  }`}
                >
                  {provider.has_api_key ? '已配置' : '未配置'}
                </span>
                {provider.id === settings.active_id && (
                  <span className="flex-none rounded-full bg-[rgba(110,168,254,0.16)] px-1.5 py-px text-[10px] text-accent">
                    当前
                  </span>
                )}
              </button>
            ))}

            <button
              className="mt-1.5 rounded-[9px] border border-dashed border-border px-2.5 py-2.5 text-left text-[12.5px] text-fg-dim hover:border-accent hover:text-accent"
              onClick={addProvider}
            >
              ＋ 新增服务商
            </button>
          </aside>

          <section className="overflow-y-auto px-[22px] py-[18px]">
            {!settings && !error && (
              <p className="px-4 py-6 text-[13px] text-fg-dim">加载中…</p>
            )}
            {error && (
              <div className="mb-3.5 rounded-lg border border-danger px-3.5 py-2.5 text-[13px] text-danger">
                {error}
              </div>
            )}

            {settings && !selected && (
              <p className="px-4 py-6 text-[13px] text-fg-dim">
                选择或新增一个服务商。
              </p>
            )}

            {settings && selected && (
              <>
                <label className={fieldClass}>
                  <span className={fieldLabelClass}>Provider 预设</span>
                  <select
                    className={inputClass}
                    value={selected.provider}
                    onChange={(event) =>
                      applyProfile(event.target.value as ProviderProfileId)
                    }
                  >
                    {catalog.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {profile.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className={fieldClass}>
                  <span className={fieldLabelClass}>名称</span>
                  <input
                    className={inputClass}
                    value={selected.name}
                    onChange={(event) => updateField('name', event.target.value)}
                    placeholder="例如：MiniMax"
                    spellCheck={false}
                  />
                </label>

                <label className={fieldClass}>
                  <span className={fieldLabelClass}>Base URL</span>
                  <input
                    className={inputClass}
                    value={selected.base_url}
                    onChange={(event) =>
                      updateField('base_url', event.target.value)
                    }
                    placeholder={
                      selectedProfile?.default_base_url ||
                      'https://gateway.example/v1'
                    }
                    spellCheck={false}
                  />
                </label>

                <label className={fieldClass}>
                  <span className={fieldLabelClass}>API Key</span>
                  <div className="flex gap-1.5">
                    <input
                      className={`${inputClass} min-w-0 flex-1`}
                      type={keyVisible ? 'text' : 'password'}
                      value={keyDraft}
                      onChange={(event) =>
                        setKeyDraft(selected.id, event.target.value)
                      }
                      placeholder={
                        selected.has_api_key
                          ? '已保存，留空则不修改'
                          : '粘贴 API Key'
                      }
                      autoComplete="new-password"
                      spellCheck={false}
                    />
                    <button
                      type="button"
                      className={ghostButtonClass}
                      onClick={() => setKeyVisible((visible) => !visible)}
                    >
                      {keyVisible ? '隐藏' : '显示'}
                    </button>
                  </div>
                </label>

                {selected.has_api_key && (
                  <div className="-mt-1 mb-3 flex flex-wrap items-center gap-2">
                    <button
                      type="button"
                      className="text-xs text-accent hover:underline"
                      onClick={() => toggleClearKey(selected.id)}
                    >
                      {clearKeys[selected.id]
                        ? '取消清除已保存的 Key'
                        : '清除已保存的 Key'}
                    </button>
                    {clearKeys[selected.id] && (
                      <span className="text-xs text-fg-dim">
                        保存后将删除该服务商的 key
                      </span>
                    )}
                  </div>
                )}

                <label className={fieldClass}>
                  <span className={fieldLabelClass}>模型</span>
                  <div className="flex flex-wrap gap-1.5">
                    <input
                      className={`${inputClass} min-w-0 flex-1 basis-40`}
                      list={datalistId}
                      value={selected.model}
                      onChange={(event) =>
                        updateField('model', event.target.value)
                      }
                      placeholder={
                        selectedProfile?.default_model ||
                        '手动输入模型 ID'
                      }
                      spellCheck={false}
                    />
                    <button
                      type="button"
                      className={ghostButtonClass}
                      onClick={() => void fetchModels(selected)}
                      disabled={modelsLoading[selected.id] === true}
                    >
                      {modelsLoading[selected.id] ? '拉取中…' : '拉取模型'}
                    </button>
                    <button
                      type="button"
                      className={ghostButtonClass}
                      onClick={() => void testConnection(selected)}
                      disabled={probe?.status === 'loading'}
                    >
                      测试连接
                    </button>
                  </div>
                  <datalist id={datalistId}>
                    {models.map((model) => (
                      <option key={model} value={model} />
                    ))}
                  </datalist>
                </label>

                {modelsByProvider[selected.id] && (
                  <p className="mt-1.5 text-xs text-fg-dim">
                    已获取 {models.length} 个模型，可直接选择或手动输入
                  </p>
                )}
                {modelErrors[selected.id] && (
                  <div className="mb-3.5 rounded-lg border border-danger px-3.5 py-2.5 text-[13px] text-danger">
                    {modelErrors[selected.id]}
                  </div>
                )}
                {probe?.status === 'loading' && (
                  <p className="mt-1.5 text-xs text-fg-dim">正在测试连接…</p>
                )}
                {probe?.status === 'ok' && probe.result && (
                  <p className="mt-2 text-xs text-ok">
                    连接成功 · {probe.result.latency_ms} ms · {probe.result.model}
                  </p>
                )}
                {probe?.status === 'error' && (
                  <div className="mb-3.5 rounded-lg border border-danger px-3.5 py-2.5 text-[13px] text-danger">
                    {probe.error}
                  </div>
                )}

                <details className="mt-4 border-t border-border pt-2.5">
                  <summary className="cursor-pointer text-xs text-fg-dim">
                    高级兼容性
                  </summary>
                  <div className="mt-2.5 grid gap-2">
                    <label className="flex items-center justify-between gap-3 text-xs text-fg-dim">
                      <span>JSON 模式</span>
                      <select
                        className={`${selectClass} max-w-[280px]`}
                        value={
                          selected.compat.json_mode === undefined ||
                          selected.compat.json_mode === null
                            ? 'auto'
                            : selected.compat.json_mode
                              ? 'on'
                              : 'off'
                        }
                        onChange={(event) =>
                          updateCompat(
                            'json_mode',
                            event.target.value === 'auto'
                              ? undefined
                              : event.target.value === 'on',
                          )
                        }
                      >
                        <option value="auto">
                          跟随预设（
                          {selectedProfile?.capabilities.json_mode
                            ? '开启'
                            : '关闭'}
                          ）
                        </option>
                        <option value="on">开启</option>
                        <option value="off">关闭</option>
                      </select>
                    </label>

                    <label className="flex items-center justify-between gap-3 text-xs text-fg-dim">
                      <span>max tokens 字段</span>
                      <select
                        className={`${selectClass} max-w-[280px]`}
                        value={selected.compat.max_tokens_field ?? 'auto'}
                        onChange={(event) =>
                          updateCompat(
                            'max_tokens_field',
                            event.target.value === 'auto'
                              ? undefined
                              : (event.target.value as MaxTokensField),
                          )
                        }
                      >
                        <option value="auto">
                          跟随预设（
                          {selectedProfile?.capabilities.max_tokens_field}
                          ）
                        </option>
                        <option value="max_tokens">max_tokens</option>
                        <option value="max_completion_tokens">
                          max_completion_tokens
                        </option>
                      </select>
                    </label>

                    <label className="flex items-center justify-between gap-3 text-xs text-fg-dim">
                      <span>Thinking 模式</span>
                      <select
                        className={`${selectClass} max-w-[280px]`}
                        value={selected.compat.thinking_mode ?? 'auto'}
                        onChange={(event) =>
                          updateCompat(
                            'thinking_mode',
                            event.target.value === 'auto'
                              ? undefined
                              : (event.target.value as ThinkingMode),
                          )
                        }
                      >
                        <option value="auto">
                          跟随预设（
                          {thinkingModeLabel(
                            selectedProfile?.capabilities.thinking_mode ??
                              'default',
                          )}
                          ）
                        </option>
                        <option value="default">默认（不发送）</option>
                        <option value="disabled">禁用</option>
                        <option value="adaptive">自适应</option>
                      </select>
                    </label>

                    <label className="flex items-center justify-between gap-3 text-xs text-fg-dim">
                      <span>Thinking 参数格式</span>
                      <select
                        className={`${selectClass} max-w-[280px]`}
                        value={selected.compat.thinking_format ?? 'auto'}
                        onChange={(event) =>
                          updateCompat(
                            'thinking_format',
                            event.target.value === 'auto'
                              ? undefined
                              : (event.target.value as ThinkingFormat),
                          )
                        }
                      >
                        <option value="auto">
                          跟随预设（
                          {selectedProfile?.capabilities.thinking_format}
                          ）
                        </option>
                        <option value="thinking">thinking</option>
                        <option value="enable_thinking">enable_thinking</option>
                      </select>
                    </label>
                  </div>
                </details>

                <div className="mt-1.5 flex gap-2">
                  <button
                    className="rounded-md border border-border px-3.5 py-1.5 text-[12.5px] text-fg-dim hover:border-accent hover:text-accent disabled:opacity-45"
                    onClick={setActive}
                    disabled={selected.id === settings.active_id}
                  >
                    {selected.id === settings.active_id
                      ? '当前使用中'
                      : '设为当前'}
                  </button>
                  <button
                    className="rounded-md border border-[rgba(255,122,122,0.45)] px-3.5 py-1.5 text-[12.5px] text-danger hover:bg-[rgba(255,122,122,0.12)] disabled:opacity-45"
                    onClick={() => removeProvider(selected.id)}
                    disabled={settings.providers.length <= 1}
                  >
                    删除
                  </button>
                </div>
              </>
            )}
          </section>
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-border px-[22px] py-3">
          <span className="text-xs text-fg-dim">
            {saved
              ? '已保存'
              : 'API key 保存在本机 credentials.json，不会写入日志。'}
          </span>
          <div className="flex gap-2">
            <button
              className="rounded-md border border-border bg-transparent px-4 py-[7px] text-fg-dim"
              onClick={onClose}
            >
              关闭
            </button>
            <button
              className="rounded-md border border-accent bg-accent px-4 py-[7px] text-white disabled:opacity-45"
              onClick={() => void submit()}
              disabled={busy || !settings || settings.providers.length === 0}
            >
              {busy ? '保存中' : '保存'}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
