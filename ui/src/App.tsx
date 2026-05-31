import React, { useState, useRef } from 'react';
import { Box, Text } from 'ink';
import { useInput } from 'ink';

// Components
import { PersistentInputBar } from './components/PersistentInputBar';
import { PaperList } from './components/PaperList';
import { PaperDetail } from './components/PaperDetail';
import { ConfigMenu } from './components/ConfigMenu';
import { LoadingScreen, CategoryProgress } from './components/LoadingSpinner';
import { Home } from './components/Home';

// Services
import { searchArxiv, quickSearch, ArxivPaper } from './services/arxiv';
import { speakText } from './services/tts';

type AppMode = 'welcome' | 'loading' | 'list' | 'detail' | 'error';

interface AppConfig {
  maxPapers: number;
  keywords: string[];
  voiceSpeed: number;
}

// ── Preset definitions ──────────────────────────────────────────

interface Preset {
  id: string;
  label: string;
  categories: string[];
}

const presets: Record<string, Preset> = {
  robotics: {
    id: 'robotics',
    label: '机器人领域',
    categories: ['cs.RO', 'cs.AI', 'cs.CV', 'cs.LG'],
  },
  ai: {
    id: 'ai',
    label: 'AI/ML',
    categories: ['cs.AI', 'cs.LG', 'cs.CL'],
  },
  cv: {
    id: 'cv',
    label: '计算机视觉',
    categories: ['cs.CV', 'cs.AI'],
  },
};

// ── App component ────────────────────────────────────────────────

const App: React.FC = () => {
  const [mode, setMode] = useState<AppMode>('welcome');
  const [papers, setPapers] = useState<ArxivPaper[]>([]);
  const [selectedPaper, setSelectedPaper] = useState<ArxivPaper | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showConfig, setShowConfig] = useState(false);
  const [apiKey] = useState(() => process.env.MINIMAX_API_KEY || '');
  const [config, setConfig] = useState<AppConfig>({
    maxPapers: 5,
    keywords: [],
    voiceSpeed: 5,
  });

  // ── Persistent input buffer ────────────────────────────────────
  const [inputBuffer, setInputBuffer] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchHistory, setSearchHistory] = useState<string[]>([]);

  // Loading state
  const [loadingMessage, setLoadingMessage] = useState('');
  const [categoryProgress, setCategoryProgress] = useState<CategoryProgress[]>([]);
  const abortRef = useRef(false);

  // ── Global keyboard input ──────────────────────────────────────

  // Ctrl+C double-press tracking
  const [ctrlCPress, setCtrlCPress] = useState(0);

  useInput((input, key) => {
    // ── Ctrl+C — double press to quit (always first) ──────────
    if ((input === 'c' || input === '\x03') && key.ctrl) {
      const now = Date.now();
      if (ctrlCPress > 0 && now - ctrlCPress < 3000) {
        process.exit(0);
      }
      setCtrlCPress(now);
      return;
    }
    // Reset ctrl+c counter after 3s
    if (ctrlCPress > 0 && Date.now() - ctrlCPress > 3000) {
      setCtrlCPress(0);
    }

    // ── Config overlay: only allow Esc / q to close ──────────
    if (showConfig) {
      if (key.escape || input === '\x1b' || ((input === 'q' || input === 'Q') && !key.ctrl && !key.meta)) {
        setShowConfig(false);
      }
      return;
    }

    // ── Esc — always back (clear buffer first) ───────────────
    if (key.escape || input === '\x1b') {
      setInputBuffer('');
      if (mode === 'welcome') { process.exit(0); return; }
      if (mode === 'list') { setMode('welcome'); return; }
      if (mode === 'detail') { setMode('list'); return; }
      if (mode === 'error') { setMode('welcome'); setError(null); return; }
      if (mode === 'loading') { abortRef.current = true; setMode('welcome'); return; }
      return;
    }

    // ── Backspace / Delete — always modify buffer ────────────
    if (key.backspace || key.delete) {
      setInputBuffer(prev => prev.slice(0, -1));
      return;
    }

    // ── Enter with buffer → search from any mode ─────────────
    if (key.return && inputBuffer.trim()) {
      const query = inputBuffer.trim();
      setInputBuffer('');

      // Check for slash commands
      const cmd = query.toLowerCase();
      if (cmd === '/robotics' || cmd === '/r') {
        addToHistory(query);
        setSearchQuery('快捷查询: 机器人领域');
        handleQuickSearch(presets.robotics);
        return;
      }
      if (cmd === '/ai' || cmd === '/a') {
        addToHistory(query);
        setSearchQuery('快捷查询: AI/ML');
        handleQuickSearch(presets.ai);
        return;
      }
      if (cmd === '/cv' || cmd === '/c') {
        addToHistory(query);
        setSearchQuery('快捷查询: 计算机视觉');
        handleQuickSearch(presets.cv);
        return;
      }

      setSearchQuery(query);
      handleSearch(query);
      return;
    }

    // ── Keys that only work when buffer is EMPTY ────────────
    if (!inputBuffer) {
      // q — back navigation
      if ((input === 'q' || input === 'Q') && !key.ctrl && !key.meta) {
        if (mode === 'welcome') { process.exit(0); return; }
        if (mode === 'list') { setMode('welcome'); return; }
        if (mode === 'detail') { setMode('list'); return; }
        if (mode === 'error') { setMode('welcome'); setError(null); return; }
        if (mode === 'loading') { abortRef.current = true; setMode('welcome'); return; }
        return;
      }

      // / — config (welcome, list, detail)
      if (input === '/') {
        if (mode === 'welcome' || mode === 'list' || mode === 'detail') {
          setShowConfig(true);
        }
        return;
      }

      // Number keys in home/welcome — select from history
      if (mode === 'welcome' && /^[1-9]$/.test(input)) {
        const num = parseInt(input);
        if (num >= 1 && num <= Math.min(searchHistory.length, 8)) {
          const query = searchHistory[num - 1];
          setSearchQuery(query);
          setInputBuffer('');
          handleSearch(query);
        }
        return;
      }

      // Enter on welcome — start search with keyboard (visual hint)
      if (key.return && mode === 'welcome') {
        // Just focus stays on the bar — nothing to do
        return;
      }

      // Number keys in list mode — handled by PaperList component
      // (expand abstract on first press, select on second)

      // s — speak in detail view
      if ((input === 's' || input === 'S') && mode === 'detail' && selectedPaper && !key.ctrl && !key.meta) {
        handleSpeak(selectedPaper.summary || selectedPaper.title);
        return;
      }

      // f — save/favorite in detail view
      if ((input === 'f' || input === 'F') && mode === 'detail' && selectedPaper && !key.ctrl && !key.meta) {
        handleSave(selectedPaper);
        return;
      }

      // b — back from detail
      if ((input === 'b' || input === 'B') && mode === 'detail' && !key.ctrl && !key.meta) {
        setMode('list');
        return;
      }

      // Error mode: any other key clears error
      if (mode === 'error') {
        setMode('welcome');
        setError(null);
        return;
      }
    }

    // ── Regular character input → append to buffer ──────────
    // In list mode, single digits are reserved for paper selection
    if (mode === 'list' && /^[1-9]$/.test(input) && !inputBuffer) {
      return;
    }
    // In detail mode, j/k/Tab/arrows/Space are used for section navigation
    if (mode === 'detail' && !inputBuffer) {
      if (key.tab || key.upArrow || key.downArrow || key.return || input === ' ') return;
      if (input === 'j' || input === 'J' || input === 'k' || input === 'K') return;
    }
    if (
      input &&
      input.length > 0 &&
      !key.ctrl &&
      !key.meta &&
      !key.tab &&
      !key.upArrow &&
      !key.downArrow &&
      !key.leftArrow &&
      !key.rightArrow &&
      !key.return
    ) {
      setInputBuffer(prev => prev + input);
    }
  });

  // ── Search history helper ──────────────────────────────────────
  const addToHistory = (query: string) => {
    setSearchHistory(prev => {
      const deduped = prev.filter(q => q !== query);
      return [query, ...deduped].slice(0, 12);
    });
  };

  // ── Search handlers ─────────────────────────────────────────────

  const handleSearch = async (query: string): Promise<void> => {
    addToHistory(query);
    setMode('loading');
    setLoadingMessage('搜索中...');
    setCategoryProgress([]);
    setError(null);

    try {
      const results = await searchArxiv(query, config.maxPapers);
      setPapers(results);
      if (results.length === 0) {
        setError('未找到论文。arXiv 可能限速，请稍后重试。');
        setMode('error');
      } else {
        setMode('list');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setMode('error');
    }
  };

  /** Quick search: use backend endpoint for reliability, show animated progress */
  const handleQuickSearch = async (preset: Preset): Promise<void> => {
    setMode('loading');
    setLoadingMessage(`${preset.label} · 查询中`);
    setPapers([]);

    const initialProgress: CategoryProgress[] = preset.categories.map(cat => ({
      category: cat,
      status: 'pending',
    }));
    setCategoryProgress(initialProgress);

    let animTimer: ReturnType<typeof setInterval> | null = null;
    let step = 0;
    animTimer = setInterval(() => {
      setCategoryProgress(prev => {
        if (step < prev.length) {
          const next = [...prev];
          next[step] = { ...next[step], status: 'fetching' };
          return next;
        }
        return prev;
      });
      step++;
    }, 1500);

    try {
      const results = await quickSearch(preset.id, config.maxPapers * 2);

      if (animTimer) clearInterval(animTimer);
      setCategoryProgress(preset.categories.map(cat => {
        const catPapers = results.filter(p =>
          p.categories.some(c => c.toLowerCase().startsWith(cat.toLowerCase().split('.')[0]))
        );
        return { category: cat, status: 'done' as const, count: catPapers.length };
      }));

      await new Promise(r => setTimeout(r, 300));

      if (results.length === 0) {
        setLoadingMessage('快捷查询无结果，尝试关键词搜索...');
        const fallback = await searchArxiv(preset.id, config.maxPapers);
        if (fallback.length === 0) {
          setError('未找到论文。arXiv 可能限速，请稍后重试。');
          setMode('error');
        } else {
          setPapers(fallback);
          setMode('list');
        }
      } else {
        setPapers(results);
        setMode('list');
      }
    } catch (err) {
      if (animTimer) clearInterval(animTimer);
      setError(err instanceof Error ? err.message : String(err));
      setMode('error');
    }
  };

  const handleSpeak = async (text: string): Promise<void> => {
    await speakText(text, config.voiceSpeed);
  };

  const handleSave = async (paper: ArxivPaper): Promise<void> => {
    const { savePaper } = await import('./services/arxiv');
    try {
      await savePaper(paper);
    } catch {
      // Silently fail — paper might already be saved
    }
  };

  const handleConfigUpdate = async (key: keyof AppConfig | 'apiKey', value: any): Promise<void> => {
    if (key === 'apiKey') {
      try {
        const { updateConfig } = await import('./services/minimax');
        await updateConfig({ api_key: value } as any);
      } catch {
        // Proceed anyway
      }
      setShowConfig(false);
      return;
    }
    setConfig(prev => ({ ...prev, [key]: value }));
    setShowConfig(false);
  };

  // ── Breadcrumb ──────────────────────────────────────────────────
  const getLocation = (): string => {
    if (showConfig) return '首页 > 配置';
    switch (mode) {
      case 'welcome': return '首页';
      case 'loading': return '首页 > 搜索中';
      case 'list': return '首页 > 搜索结果';
      case 'detail':
        if (selectedPaper) {
          const t = selectedPaper.title;
          return `首页 > 搜索结果 > ${t.length > 40 ? t.slice(0, 40) + '...' : t}`;
        }
        return '首页 > 搜索结果 > 论文详情';
      case 'error': return '首页 > 提示';
    }
  };

  // ── Render ──────────────────────────────────────────────────────

  if (showConfig) {
    return (
      <Box flexDirection="column">
        <ConfigMenu
          config={config}
          apiKey={apiKey}
          onUpdate={handleConfigUpdate}
          onClose={() => setShowConfig(false)}
        />
        <PersistentInputBar buffer={inputBuffer} location={getLocation()} />
      </Box>
    );
  }

  let content: React.ReactNode = null;

  switch (mode) {
    case 'welcome':
      content = (
        <Home
          onStart={() => setMode('welcome')}
          onQuickSearch={(id) => {
            if (presets[id]) {
              addToHistory(`/${id}`);
              setSearchQuery(`快捷查询: ${presets[id].label}`);
              setInputBuffer('');
              handleQuickSearch(presets[id]);
            }
          }}
          history={searchHistory}
          onHistorySelect={(query) => {
            setSearchQuery(query);
            setInputBuffer('');
            handleSearch(query);
          }}
        />
      );
      break;

    case 'loading':
      content = (
        <LoadingScreen
          message={loadingMessage}
          categories={categoryProgress}
          hints={[
            '中文查询会自动翻译为英文关键词搜索',
            '搜索较慢时可减少最大论文数（按 / 进入配置）',
            '使用 /robotics、/ai、/cv 可快捷查询分类',
            'arXiv API 有速率限制，频繁搜索可能暂时无结果',
          ]}
        />
      );
      break;

    case 'list':
      content = (
        <PaperList
          papers={papers}
          onSelect={(paper) => { setSelectedPaper(paper); setMode('detail'); }}
        />
      );
      break;

    case 'detail':
      content = selectedPaper ? (
        <PaperDetail
          paper={selectedPaper}
          onBack={() => setMode('list')}
          onSpeak={handleSpeak}
          onSave={handleSave}
          apiKey={apiKey}
        />
      ) : null;
      break;

    case 'error':
      content = (
        <Box flexDirection="column" padding={1}>
          <Box marginBottom={1}>
            <Text bold color="yellow">提示</Text>
          </Box>
          <Text>{error}</Text>
          <Box marginTop={1}>
            <Text dimColor>按任意键返回首页    q 退出</Text>
          </Box>
        </Box>
      );
      break;
  }

  return (
    <Box flexDirection="column">
      {searchQuery && mode !== 'welcome' && (
        <Box paddingLeft={1} paddingRight={1} marginBottom={1}>
          <Text dimColor>❯ {searchQuery}</Text>
        </Box>
      )}
      {content}
      <PersistentInputBar buffer={inputBuffer} location={getLocation()} />
      {ctrlCPress > 0 && (Date.now() - ctrlCPress < 3000) && (
        <Box>
          <Text dimColor>  再次按 Ctrl+C 退出</Text>
        </Box>
      )}
    </Box>
  );
};

export default App;
