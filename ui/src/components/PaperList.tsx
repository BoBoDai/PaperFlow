import React from 'react';
import { Text, Box } from 'ink';
import { ArxivPaper } from '../services/arxiv';

interface PaperListProps {
  papers: ArxivPaper[];
  onSelect: (paper: ArxivPaper) => void;
}

export const PaperList: React.FC<PaperListProps> = ({ papers, onSelect }) => {
  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">论文列表</Text>
        <Text dimColor> ({papers.length} 篇)</Text>
      </Box>

      <Box borderStyle="round" flexDirection="column" padding={1}>
        {papers.map((paper, index) => (
          <Box key={paper.id} flexDirection="column" marginY={1}>
            <Text bold>
              {index + 1}. {paper.title}
            </Text>
            <Text dimColor>{paper.authors.join(', ')}</Text>
            <Text dimColor>{paper.published}</Text>
          </Box>
        ))}
      </Box>

      <Text dimColor>按 1-9 选择论文</Text>
      <Text dimColor>按 / 显示配置，按 Esc 返回</Text>
    </Box>
  );
};
