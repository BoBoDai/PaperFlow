import React, { useState, useEffect } from 'react';
import { Text, Box } from 'ink';
import { ArxivPaper } from '../services/arxiv';
import { summarizePaper, translatePaper, PaperSummary, PaperTranslation } from '../services/minimax';

interface PaperDetailProps {
  paper: ArxivPaper;
  onBack: () => void;
  onSpeak: (text: string) => void;
  onSave: (paper: ArxivPaper) => void;
  apiKey: string;
}

const Divider: React.FC<{ label?: string }> = ({ label }) => (
  <Box>
    <Text dimColor>{label ? `─── ${label} ───` : '──────────────────────────────────'}</Text>
  </Box>
);

export const PaperDetail: React.FC<PaperDetailProps> = ({ paper, onBack, onSpeak, onSave, apiKey }) => {
  const [summary, setSummary] = useState<PaperSummary | null>(null);
  const [translation, setTranslation] = useState<PaperTranslation | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (paper) {
      loadContent();
    }
  }, [paper]);

  const loadContent = async (): Promise<void> => {
    setIsLoading(true);
    setError(null);
    try {
      // Fetch summary and translation in parallel
      const [sumResult, transResult] = await Promise.all([
        summarizePaper(paper, apiKey),
        translatePaper(paper.title, paper.summary || ''),
      ]);
      setSummary(sumResult);
      if (transResult.success) {
        setTranslation(transResult);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const sourceText = paper.venue
    ? paper.venue
    : paper.source === 'arxiv'
      ? `arXiv · ${paper.id}`
      : `${paper.source} · ${paper.id}`;

  return (
    <Box flexDirection="column" padding={1}>
      <Divider label="论文详情" />

      {/* Title — bilingual */}
      <Box marginY={1} flexDirection="column">
        <Text bold>{paper.title}</Text>
        {translation?.title_cn ? (
          <Text color="cyan">{translation.title_cn}</Text>
        ) : null}
      </Box>

      {/* Metadata */}
      <Box flexDirection="column">
        <Box>
          <Text dimColor>作者  </Text>
          <Text>{paper.authors.join(', ')}</Text>
        </Box>
        <Box>
          <Text dimColor>日期  </Text>
          <Text>{paper.published}</Text>
        </Box>
        <Box>
          <Text dimColor>分类  </Text>
          <Text>{paper.categories.join(', ') || '--'}</Text>
        </Box>
        <Box>
          <Text dimColor>来源  </Text>
          <Text>{sourceText}</Text>
        </Box>
        {paper.pdfUrl ? (
          <Box>
            <Text dimColor>PDF   </Text>
            <Text dimColor>{paper.pdfUrl}</Text>
          </Box>
        ) : null}
      </Box>

      {/* Abstract — bilingual */}
      <Box marginTop={1}>
        <Divider label="摘要" />
      </Box>
      <Box marginTop={1} flexDirection="column">
        <Text>{paper.summary || '暂无摘要'}</Text>
        {translation?.abstract_cn ? (
          <Box marginTop={1}>
            <Text color="cyan">{translation.abstract_cn}</Text>
          </Box>
        ) : null}
      </Box>

      {/* LLM analysis */}
      <Box marginTop={1}>
        <Divider label="智能分析" />
      </Box>
      <Box marginTop={1} flexDirection="column">
        {isLoading && <Text dimColor>  正在生成...</Text>}
        {error && <Text color="red">  失败: {error}</Text>}
        {!isLoading && !error && !summary && (
          <Text dimColor>  暂无分析结果</Text>
        )}

        {summary && (
          <>
            {summary.short_summary ? (
              <Box flexDirection="column" marginBottom={0}>
                <Box>
                  <Text dimColor>概述  </Text>
                  <Text>{summary.short_summary}</Text>
                </Box>
              </Box>
            ) : null}

            {summary.detailed_summary ? (
              <Box flexDirection="column" marginBottom={0}>
                <Box>
                  <Text dimColor>分析  </Text>
                  <Text>{summary.detailed_summary}</Text>
                </Box>
              </Box>
            ) : null}

            {summary.key_points.length > 0 ? (
              <Box flexDirection="column">
                <Text dimColor>关键点</Text>
                {summary.key_points.map((point, i) => (
                  <Text key={i} dimColor>    · {point}</Text>
                ))}
              </Box>
            ) : null}
          </>
        )}
      </Box>

      {/* Keyboard hints */}
      <Box marginTop={1}>
        <Text dimColor>f 收藏    s 语音播报    b 返回    / 配置</Text>
      </Box>
    </Box>
  );
};
