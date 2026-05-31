import React, { useState } from 'react';
import { Text, Box, useInput, useStdout } from 'ink';
import { ArxivPaper } from '../services/arxiv';

interface PaperListProps {
  papers: ArxivPaper[];
  onSelect: (paper: ArxivPaper) => void;
  title?: string;
}

const truncate = (text: string, maxLen: number): string => {
  const clean = text.replace(/\s+/g, ' ').trim();
  if (clean.length <= maxLen) return clean;
  return clean.slice(0, maxLen) + '...';
};

/** Map arXiv category to a color */
const categoryColor = (cat: string): string | undefined => {
  const prefix = cat.split('.').pop()?.toUpperCase() || cat;
  switch (prefix) {
    case 'RO': return 'yellow';
    case 'CV': return 'green';
    case 'AI': return 'cyan';
    case 'LG': return 'magenta';
    case 'CL': return 'blue';
    case 'NE': return 'red';
    default: return undefined;
  }
};

export const PaperList: React.FC<PaperListProps> = ({ papers, onSelect, title }) => {
  const { stdout } = useStdout();
  const termWidth = stdout?.columns ?? 80;

  // Track which papers have abstract expanded
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  useInput((input, _key) => {
    // Number keys: expand abstract on first press, select on second
    if (/^[1-9]$/.test(input)) {
      const num = parseInt(input);
      if (num < 1 || num > papers.length) return;

      const idx = num - 1;
      if (expanded.has(idx)) {
        // Already expanded → go to detail
        onSelect(papers[idx]);
      } else {
        // Not expanded → expand
        toggleExpand(idx);
      }
    }
  });

  const toggleExpand = (index: number) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const headerText = title || '论文列表';

  return (
    <Box flexDirection="column" paddingLeft={1} paddingRight={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Text dimColor>─── {headerText} · {papers.length} 篇</Text>
      </Box>

      {papers.length === 0 ? (
        <Box flexDirection="column" marginBottom={1}>
          <Text dimColor>  未找到论文</Text>
          <Text dimColor>  提示: arXiv API 可能限速，请稍后重试</Text>
          <Text dimColor>  也可尝试输入英文关键词或 /ai 快捷查询</Text>
        </Box>
      ) : (
        <Box flexDirection="column">
          {papers.map((paper, index) => {
            const isExpanded = expanded.has(index);
            const num = String(index + 1).padStart(2, ' ');
            const catLabel = paper.categories[0] || '--';
            const catColor = categoryColor(catLabel);
            const dateShort = paper.published.length > 7
              ? paper.published.slice(2, 7)
              : paper.published;
            const authorsShort = truncate(paper.authors.join(', '), 45);

            return (
              <Box key={paper.id} flexDirection="column" marginBottom={0}>
                {/* Top divider with number and category badge */}
                <Box>
                  <Text dimColor>──</Text>
                  <Text color="yellow" bold> {num} </Text>
                  <Text dimColor>──</Text>
                  <Text color={catColor} bold> {catLabel} </Text>
                  <Text dimColor>· {dateShort} </Text>
                  <Text dimColor>
                    {'─'.repeat(Math.max(termWidth - num.length - catLabel.length - dateShort.length - 10, 4))}
                  </Text>
                </Box>

                {/* Title */}
                <Box paddingLeft={2}>
                  <Text bold>{paper.title}</Text>
                </Box>

                {/* Authors */}
                <Box paddingLeft={2}>
                  <Text dimColor>{authorsShort}</Text>
                </Box>

                {/* Expand / select hint */}
                <Box paddingLeft={2}>
                  <Text color={isExpanded ? 'green' : 'cyan'}>
                    {isExpanded ? '▾' : '▸'}
                  </Text>
                  <Text dimColor>
                    {isExpanded
                      ? ` 收起 · 按 ${index + 1} 进入详情`
                      : ` 展开 · 按 ${index + 1} 查看摘要`}
                  </Text>
                </Box>

                {/* Expanded abstract */}
                {isExpanded && paper.summary && (
                  <Box flexDirection="column" paddingLeft={2} marginTop={1}>
                    <Text>{paper.summary.replace(/\s+/g, ' ').trim()}</Text>
                  </Box>
                )}

                {/* Inter-paper spacing */}
                <Box>
                  <Text> </Text>
                </Box>
              </Box>
            );
          })}
        </Box>
      )}

      {/* Keyboard hints */}
      <Box marginTop={1}>
        <Text dimColor>
          1-{Math.min(papers.length, 9)} 展开/选择    / 配置    q 返回
        </Text>
      </Box>
    </Box>
  );
};
