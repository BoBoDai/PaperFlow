import React, { useState, useEffect } from 'react';
import { Box, Text } from 'ink';
import { useInput } from 'ink';

// Components
import { SearchPrompt } from './components/SearchPrompt';
import { PaperList } from './components/PaperList';
import { PaperDetail } from './components/PaperDetail';
import { ConfigMenu } from './components/ConfigMenu';
import { LoadingSpinner } from './components/LoadingSpinner';
import { Welcome } from './components/Welcome';

// Services
import { searchArxiv, ArxivPaper } from './services/arxiv';
import { translateQuery } from './services/minimax';
import { speakText } from './services/tts';

type AppMode = 'welcome' | 'search' | 'loading' | 'translating' | 'list' | 'detail' | 'error';

interface AppConfig {
  maxPapers: number;
  keywords: string[];
  voiceSpeed: number;
}

const App: React.FC = () => {
  const [mode, setMode] = useState<AppMode>('welcome');
  const [searchQuery, setSearchQuery] = useState('');
  const [papers, setPapers] = useState<ArxivPaper[]>([]);
  const [selectedPaper, setSelectedPaper] = useState<ArxivPaper | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showConfig, setShowConfig] = useState(false);
  const [inputReady, setInputReady] = useState(false);
  const [apiKey] = useState(() => process.env.MINIMAX_API_KEY || '');
  const [config, setConfig] = useState<AppConfig>({
    maxPapers: 5,
    keywords: [],
    voiceSpeed: 5,
  });

  // Handle keyboard input - only active when input is ready
  useInput((input, key) => {
    if (!inputReady) return;

    // Escape - go back or close config
    if (key.escape) {
      if (showConfig) {
        setShowConfig(false);
      } else if (mode === 'search') {
        setMode('welcome');
      } else if (mode === 'list') {
        setMode('search');
      } else if (mode === 'detail') {
        setMode('list');
      } else if (mode === 'error') {
        setMode('search');
      }
      return;
    }

    // Slash - show config
    if (input === '/') {
      if (mode === 'search' || mode === 'list') {
        setShowConfig(true);
      }
      return;
    }

    // Enter - start search from welcome
    if (key.return && mode === 'welcome') {
      setMode('search');
      return;
    }

    // Enter - submit search
    if (key.return && mode === 'search' && searchQuery) {
      handleSearch(searchQuery);
      return;
    }

    // Number keys 1-9 - select paper
    if (mode === 'list') {
      const num = parseInt(input);
      if (num >= 1 && num <= papers.length) {
        setSelectedPaper(papers[num - 1]);
        setMode('detail');
      }
      return;
    }

    // s key - speak
    if (input === 's' && mode === 'detail' && selectedPaper) {
      handleSpeak(selectedPaper.summary);
    }
  }, { isActive: inputReady });

  // Mark input as ready after first render
  useEffect(() => {
    setInputReady(true);
  }, []);

  const handleSearch = async (query: string): Promise<void> => {
    setMode('loading');

    try {
      let searchTerm = query;
      if (/[\u4e00-\u9fa5]/.test(query)) {
        setMode('translating');
        searchTerm = await translateQuery(query, apiKey);
      }

      setMode('loading');
      const results = await searchArxiv(searchTerm, config.maxPapers);
      setPapers(results);
      setMode('list');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setMode('error');
    }
  };

  const handlePaperSelect = (paper: ArxivPaper): void => {
    setSelectedPaper(paper);
    setMode('detail');
  };

  const handleSpeak = async (text: string): Promise<void> => {
    await speakText(text, config.voiceSpeed);
  };

  const handleConfigUpdate = (key: keyof AppConfig, value: AppConfig[keyof AppConfig]): void => {
    setConfig(prev => ({ ...prev, [key]: value }));
    setShowConfig(false);
  };

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
      return <Welcome onStart={() => setMode('search')} />;

    case 'search':
      return (
        <SearchPrompt
          value={searchQuery}
          onChange={setSearchQuery}
          onSubmit={() => searchQuery && handleSearch(searchQuery)}
        />
      );

    case 'loading':
      return <LoadingSpinner message="搜索论文中..." />;

    case 'translating':
      return <LoadingSpinner message="翻译查询中..." />;

    case 'list':
      return (
        <PaperList
          papers={papers}
          onSelect={handlePaperSelect}
        />
      );

    case 'detail':
      return selectedPaper ? (
        <PaperDetail
          paper={selectedPaper}
          onBack={() => setMode('list')}
          onSpeak={handleSpeak}
          apiKey={apiKey}
        />
      ) : null;

    case 'error':
      return (
        <Box flexDirection="column" padding={1}>
          <Text color="red">错误: {error}</Text>
          <Text dimColor>按 Esc 返回搜索</Text>
        </Box>
      );

    default:
      return null;
  }
};

export default App;
