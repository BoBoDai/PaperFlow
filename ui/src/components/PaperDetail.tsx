import React, { useState, useEffect } from 'react';
import { Text, Box, useInput } from 'ink';
import { ArxivPaper } from '../services/arxiv';
import { summarizePaper, translatePaper, PaperSummary, PaperTranslation } from '../services/minimax';

interface PaperDetailProps {
  paper: ArxivPaper;
  onBack: () => void;
  onSpeak: (text: string) => void;
  onSave: (paper: ArxivPaper) => void;
  apiKey: string;
}

type Section = 'abstract' | 'translation' | 'analysis';

export const PaperDetail: React.FC<PaperDetailProps> = ({
  paper, onBack, onSpeak, onSave, apiKey,
}) => {
  const [summary, setSummary] = useState<PaperSummary | null>(null);
  const [translation, setTranslation] = useState<PaperTranslation | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Collapsible sections
  const [expanded, setExpanded] = useState<Set<Section>>(new Set(['abstract']));
  const [activeSection, setActiveSection] = useState<Section>('abstract');

  const sections: Section[] = ['abstract', 'translation', 'analysis'];

  useEffect(() => {
    if (paper) {
      loadContent();
    }
  }, [paper]);

  // Keyboard: Tab / j/k to navigate sections, Enter / Space to toggle
  useInput((input, key) => {
    if (key.tab || input === 'j' || input === 'J' || key.downArrow) {
      const idx = sections.indexOf(activeSection);
      setActiveSection(sections[(idx + 1) % sections.length]);
      return;
    }
    if (input === 'k' || input === 'K' || key.upArrow) {
      const idx = sections.indexOf(activeSection);
      setActiveSection(sections[(idx - 1 + sections.length) % sections.length]);
      return;
    }
    if (key.return || input === ' ') {
      toggleSection(activeSection);
      return;
    }
  });

  const loadContent = async (): Promise<void> => {
    setIsLoading(true);
    setError(null);
    try {
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

  const toggleSection = (section: Section) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(section)) {
        next.delete(section);
      } else {
        next.add(section);
      }
      return next;
    });
  };

  const isExpanded = (section: Section) => expanded.has(section);
  const isActive = (section: Section) => section === activeSection;

  const sectionLabel = (section: Section): string => {
    switch (section) {
      case 'abstract': return '摘要';
      case 'translation': return '中文翻译';
      case 'analysis': return 'AI 分析';
    }
  };

  const sourceText = paper.venue
    ? paper.venue
    : paper.source === 'arxiv'
      ? `arXiv · ${paper.id}`
      : `${paper.source} · ${paper.id}`;

  return (
    <Box flexDirection="column" padding={1}>
      {/* Title — bilingual */}
      <Box marginBottom={1} flexDirection="column">
        <Text bold>{paper.title}</Text>
        {translation?.title_cn ? (
          <Text color="cyan">{translation.title_cn}</Text>
        ) : null}
      </Box>

      {/* Metadata — always visible */}
      <Box flexDirection="column" marginBottom={1}>
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

      {/* ── Abstract section ── */}
      <Box marginBottom={0}>
        <Text dimColor>─── </Text>
        <Text color={isActive('abstract') ? 'yellow' : undefined} bold={isActive('abstract')}>
          {isExpanded('abstract') ? '▾' : '▸'} {sectionLabel('abstract')}
        </Text>
        {isActive('abstract') && <Text color="yellow" dimColor> ◄</Text>}
      </Box>
      {isExpanded('abstract') && (
        <Box flexDirection="column" marginBottom={1} paddingLeft={2}>
          <Text>{paper.summary || '暂无摘要'}</Text>
        </Box>
      )}

      {/* ── Translation section ── */}
      <Box marginBottom={0}>
        <Text dimColor>─── </Text>
        <Text color={isActive('translation') ? 'yellow' : undefined} bold={isActive('translation')}>
          {isExpanded('translation') ? '▾' : '▸'} {sectionLabel('translation')}
        </Text>
        {isActive('translation') && <Text color="yellow" dimColor> ◄</Text>}
        {!translation && !isLoading && (
          <Text dimColor>  (加载中...)</Text>
        )}
      </Box>
      {isExpanded('translation') && translation?.abstract_cn && (
        <Box flexDirection="column" marginBottom={1} paddingLeft={2}>
          <Text color="cyan">{translation.abstract_cn}</Text>
        </Box>
      )}
      {isExpanded('translation') && !translation && !isLoading && (
        <Box paddingLeft={2}>
          <Text dimColor>暂无翻译</Text>
        </Box>
      )}

      {/* ── AI Analysis section ── */}
      <Box marginBottom={0}>
        <Text dimColor>─── </Text>
        <Text color={isActive('analysis') ? 'yellow' : undefined} bold={isActive('analysis')}>
          {isExpanded('analysis') ? '▾' : '▸'} {sectionLabel('analysis')}
        </Text>
        {isActive('analysis') && <Text color="yellow" dimColor> ◄</Text>}
        {isLoading && <Text dimColor>  生成中...</Text>}
        {error && <Text color="red">  失败</Text>}
      </Box>
      {isExpanded('analysis') && summary && (
        <Box flexDirection="column" paddingLeft={2}>
          {summary.short_summary ? (
            <Box marginBottom={0}>
              <Text dimColor>概述  </Text>
              <Text>{summary.short_summary}</Text>
            </Box>
          ) : null}
          {summary.detailed_summary ? (
            <Box marginBottom={0}>
              <Text dimColor>分析  </Text>
              <Text>{summary.detailed_summary}</Text>
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
        </Box>
      )}
      {isExpanded('analysis') && !summary && !isLoading && !error && (
        <Box paddingLeft={2}>
          <Text dimColor>暂无分析结果</Text>
        </Box>
      )}

      {/* Keyboard hints */}
      <Box marginTop={1}>
        <Text dimColor>
          Tab/j/k 切换分区    Enter 展开/折叠    f 收藏    s 语音    q 返回    / 配置
        </Text>
      </Box>
    </Box>
  );
};
