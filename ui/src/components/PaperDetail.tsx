import React, { useState, useEffect } from 'react';
import { Text, Box, Spacer } from 'ink';
import { ArxivPaper } from '../services/arxiv';
import { summarizePaper, PaperSummary } from '../services/minimax';

interface PaperDetailProps {
  paper: ArxivPaper;
  onBack: () => void;
  onSpeak: (text: string) => void;
  apiKey: string;
}

export const PaperDetail: React.FC<PaperDetailProps> = ({ paper, onBack, apiKey }) => {
  const [summary, setSummary] = useState<PaperSummary | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (paper && apiKey) {
      loadSummary();
    }
  }, [paper, apiKey]);

  const loadSummary = async (): Promise<void> => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await summarizePaper(paper, apiKey);
      setSummary(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">{paper.title}</Text>
      </Box>

      <Box borderStyle="round" padding={1} flexDirection="column">
        <Text dimColor>作者: {paper.authors.join(', ')}</Text>
        <Text dimColor>发表: {paper.published}</Text>
        <Text dimColor>分类: {paper.categories.join(', ')}</Text>
      </Box>

      <Spacer />

      {isLoading && <Text color="yellow">正在生成摘要...</Text>}

      {error && <Text color="red">错误: {error}</Text>}

      {summary && (
        <Box flexDirection="column" gap={1}>
          <Text bold>摘要:</Text>
          <Text>{summary.detailed_summary}</Text>

          {summary.key_points.length > 0 && (
            <>
              <Spacer />
              <Text bold>关键点:</Text>
              {summary.key_points.map((point, i) => (
                <Text key={i}>- {point}</Text>
              ))}
            </>
          )}
        </Box>
      )}

      <Spacer />

      <Box flexDirection="column" gap={1}>
        <Text dimColor>按 s 语音播报</Text>
        <Text dimColor>按 / 显示配置</Text>
        <Text dimColor>按 Esc 返回</Text>
      </Box>
    </Box>
  );
};
