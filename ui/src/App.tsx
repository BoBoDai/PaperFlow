import React, { useState, useRef } from 'react';
import { Box, Text } from 'ink';
import { useInput } from 'ink';

// Components
import { SearchPrompt } from './components/SearchPrompt';
import { PaperList } from './components/PaperList';
import { PaperDetail } from './components/PaperDetail';
import { ConfigMenu } from './components/ConfigMenu';
import { LoadingScreen, CategoryProgress } from './components/LoadingSpinner';
import { Welcome } from './components/Welcome';

// Services
import { searchArxiv, quickSearch, ArxivPaper } from './services/arxiv';
import { translateQuery } from './services/minimax';
import { speakText } from './services/tts';

type AppMode = 'welcome' | 'search' | 'loading' | 'translating' | 'list' | 'detail' | 'error';

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
  const [searchQuery, setSearchQuery] = useState('');
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

  // Loading state
  const [loadingMessage, setLoadingMessage] = useState('');
  const [categoryProgress, setCategoryProgress] = useState<CategoryProgress[]>([]);
  const abortRef = useRef(false);

  // ── Global keyboard input (non-search modes) ────────────────────
  // Search mode input is handled by SearchPrompt component

  useInput((input, key) => {
    // Don't handle text input when SearchPrompt is active
    // (search mode without config overlay)
    if (mode === 'search' && !showConfig) return;

    // In error mode, any non-special key goes to search
    if (mode === 'error' && !showConfig) {
      if (key.escape) { setMode('welcome'); return; }
      if (input === '/') { setShowConfig(true); return; }
      // Any other key → go to search
      setMode('search');
      setSearchQuery('');
      setError(null);
      return;
    }

    // Escape
    if (key.escape) {
      if (showConfig) { setShowConfig(false); return; }
      if (mode === 'list') { setMode('search'); setSearchQuery(''); return; }
      if (mode === 'detail') { setMode('list'); return; }
      return;
    }

    // q - quit from welcome
    if ((input === 'q' || input === 'Q') && mode === 'welcome') {
      process.exit(0);
    }

    // / - config (only in welcome and list modes)
    if (input === '/' && !showConfig) {
      if (mode === 'welcome' || mode === 'list') {
        setShowConfig(true);
      }
      return;
    }

    // Quick presets on welcome
    if (mode === 'welcome') {
      if (input === 'r' || input === 'R') { handleQuickSearch(presets.robotics); return; }
      if (input === 'a' || input === 'A') { handleQuickSearch(presets.ai); return; }
      if (input === 'c' || input === 'C') { handleQuickSearch(presets.cv); return; }
    }

    // Enter on welcome → go to search
    if (key.return && mode === 'welcome') {
      setMode('search');
      return;
    }

    // Number keys - select paper from list
    if (mode === 'list') {
      const num = parseInt(input);
      if (num >= 1 && num <= papers.length) {
        setSelectedPaper(papers[num - 1]);
        setMode('detail');
      }
      return;
    }

    // s - speak in detail view
    if (input === 's' && mode === 'detail' && selectedPaper) {
      handleSpeak(selectedPaper.summary || selectedPaper.title);
      return;
    }

    // f - save/favorite in detail view
    if (input === 'f' && mode === 'detail' && selectedPaper) {
      handleSave(selectedPaper);
      return;
    }

    // b - back from detail
    if (input === 'b' && mode === 'detail') {
      setMode('list');
      return;
    }
  });

  // ── Search handlers ─────────────────────────────────────────────

  const handleSearch = async (query: string): Promise<void> => {
    setMode('loading');
    setLoadingMessage('搜索中...');
    setCategoryProgress([]);
    setError(null);

    try {
      let term = query;
      const isChinese = /[一-龥]/.test(query);

      if (isChinese) {
        setMode('translating');
        const result = await translateQuery(query, apiKey);
        term = result.text;

        if (!result.translated && !apiKey) {
          setError('未配置 API Key，中文翻译不可用。请设置 MINIMAX_API_KEY 或使用英文关键词/快捷查询（按 r）。');
          setMode('error');
          return;
        }
      }

      setMode('loading');
      const results = await searchArxiv(term, config.maxPapers);
      setPapers(results);
      // If no results, go back to search mode so user can retry immediately
      if (results.length === 0) {
        setError('未找到论文。arXiv 可能限速，请稍后重试，或按 r 快速查询。');
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

    // Show animated progress for each category
    const initialProgress: CategoryProgress[] = preset.categories.map(cat => ({
      category: cat,
      status: 'pending',
    }));
    setCategoryProgress(initialProgress);

    // Animate progress: mark categories as fetching one by one
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
      // Mark all as done
      setCategoryProgress(preset.categories.map(cat => {
        const catPapers = results.filter(p =>
          p.categories.some(c => c.toLowerCase().startsWith(cat.toLowerCase().split('.')[0]))
        );
        return { category: cat, status: 'done' as const, count: catPapers.length };
      }));

      await new Promise(r => setTimeout(r, 300));

      if (results.length === 0) {
        // Fallback: try a keyword search
        setLoadingMessage('快捷查询无结果，尝试关键词搜索...');
        const fallback = await searchArxiv(preset.id, config.maxPapers);
        if (fallback.length === 0) {
          setError('未找到论文。arXiv 可能限速，请稍后重试，或手动输入英文关键词。');
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
      // Save API key to backend config file
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

  // ── Render ──────────────────────────────────────────────────────

  if (showConfig) {
    return (
      <ConfigMenu
        config={config}
        apiKey={apiKey}
        onUpdate={handleConfigUpdate}
        onClose={() => setShowConfig(false)}
      />
    );
  }

  switch (mode) {
    case 'welcome':
      return (
        <Welcome
          onStart={() => setMode('search')}
          onQuickSearch={(id) => presets[id] && handleQuickSearch(presets[id])}
        />
      );

    case 'search':
      return (
        <SearchPrompt
          value={searchQuery}
          onChange={setSearchQuery}
          onSubmit={(query) => {
            // Check for quick commands
            const cmd = query.toLowerCase();
            if (cmd === '/robotics' || cmd === '/r') { handleQuickSearch(presets.robotics); return; }
            if (cmd === '/ai' || cmd === '/a') { handleQuickSearch(presets.ai); return; }
            if (cmd === '/cv' || cmd === '/c') { handleQuickSearch(presets.cv); return; }
            handleSearch(query);
          }}
          onCancel={() => setMode('welcome')}
        />
      );

    case 'loading':
      return (
        <LoadingScreen
          message={loadingMessage}
          categories={categoryProgress}
        />
      );

    case 'translating':
      return (
        <LoadingScreen
          message="翻译查询..."
          categories={[{ category: '中译英', status: 'fetching' }]}
        />
      );

    case 'list':
      return (
        <PaperList
          papers={papers}
          onSelect={(paper) => { setSelectedPaper(paper); setMode('detail'); }}
        />
      );

    case 'detail':
      return selectedPaper ? (
        <PaperDetail
          paper={selectedPaper}
          onBack={() => setMode('list')}
          onSpeak={handleSpeak}
          onSave={handleSave}
          apiKey={apiKey}
        />
      ) : null;

    case 'error':
      return (
        <Box flexDirection="column" padding={1}>
          <Box marginBottom={1}>
            <Text bold color="yellow">提示</Text>
          </Box>
          <Text>{error}</Text>
          <Box marginTop={1}>
            <Text dimColor>按任意键重新搜索    Esc 返回首页</Text>
          </Box>
        </Box>
      );

    default:
      return null;
  }
};

export default App;
