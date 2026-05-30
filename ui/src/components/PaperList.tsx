import React from 'react';
import { Text, Box } from 'ink';
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

const Divider: React.FC<{ label: string }> = ({ label }) => (
  <Box>
    <Text dimColor>─── {label} ───</Text>
  </Box>
);

export const PaperList: React.FC<PaperListProps> = ({ papers, onSelect, title }) => {
  const headerText = title || '论文列表';

  return (
    <Box flexDirection="column" padding={1}>
      {/* Header with thin separator */}
      <Divider label={`${headerText} · ${papers.length} 篇`} />

      <Box marginTop={1} flexDirection="column">
        {papers.length === 0 ? (
          <Box flexDirection="column">
            <Text dimColor>  未找到论文</Text>
            <Text dimColor>  提示: arXiv API 可能限速，请稍后重试</Text>
            <Text dimColor>  也可按 r 一键查询机器人领域最新论文</Text>
          </Box>
        ) : (
          papers.map((paper, index) => (
            <Box key={paper.id} flexDirection="column" marginBottom={1}>
              {/* Title line: number + title */}
              <Box>
                <Text color="yellow" bold>
                  {index + 1 < 10 ? ` ${index + 1}` : index + 1}
                </Text>
                <Text>  </Text>
                <Text bold>{paper.title}</Text>
              </Box>

              {/* Metadata line */}
              <Box>
                <Text>     </Text>
                <Text dimColor>
                  {paper.categories.join(', ')} · {paper.published}
                  {paper.authors.length > 0 ? ` · ${truncate(paper.authors.join(', '), 50)}` : ''}
                </Text>
              </Box>

              {/* Abstract preview */}
              {paper.summary ? (
                <Box>
                  <Text>     </Text>
                  <Text dimColor>{truncate(paper.summary, 90)}</Text>
                </Box>
              ) : null}
            </Box>
          ))
        )}
      </Box>

      {/* Keyboard hints */}
      <Box marginTop={1}>
        <Text dimColor>
          1-{Math.min(papers.length, 9)} 选择    / 配置    q 返回
        </Text>
      </Box>
    </Box>
  );
};
