import React from 'react';
import { Text, Box } from 'ink';

interface HomeProps {
  /** Called when user presses Enter with empty buffer (placeholder for focus) */
  onStart: () => void;
  /** Quick search by preset id */
  onQuickSearch: (preset: string) => void;
  /** Recent search queries (newest first), max ~8 shown */
  history: string[];
  /** Re-run a history query */
  onHistorySelect: (query: string) => void;
}

const presets = [
  { key: 'r', id: 'robotics', label: '机器人', cats: 'cs.RO, cs.AI, cs.CV, cs.LG' },
  { key: 'a', id: 'ai', label: 'AI/ML', cats: 'cs.AI, cs.LG, cs.CL' },
  { key: 'c', id: 'cv', label: '视觉', cats: 'cs.CV, cs.AI' },
];

export const Home: React.FC<HomeProps> = ({ onStart, onQuickSearch, history, onHistorySelect }) => {
  return (
    <Box flexDirection="column" padding={1}>
      {/* Title */}
      <Box marginBottom={1}>
        <Text bold color="cyan">PaperFlow</Text>
        <Text dimColor> — 学术论文助手</Text>
      </Box>

      {/* Quick search section */}
      <Box flexDirection="column" marginBottom={1}>
        <Text bold>快捷查询</Text>
        {presets.map((p) => (
          <Box key={p.id}>
            <Text>  </Text>
            <Text color="yellow" bold>{p.key}</Text>
            <Text>  {p.label}</Text>
            <Text dimColor>     {p.cats}</Text>
          </Box>
        ))}
      </Box>

      {/* Search history */}
      {history.length > 0 && (
        <Box flexDirection="column" marginBottom={1}>
          <Text bold>搜索历史</Text>
          {history.slice(0, 8).map((query, i) => (
            <Box key={i}>
              <Text>  </Text>
              <Text color="yellow" bold>{i + 1}</Text>
              <Text>  </Text>
              <Text>{query}</Text>
            </Box>
          ))}
          <Box marginTop={0}>
            <Text dimColor>  按数字键重新搜索</Text>
          </Box>
        </Box>
      )}

      {/* Hint line */}
      <Box>
        <Text dimColor>/ 配置    </Text>
        <Text dimColor>q 退出</Text>
      </Box>
    </Box>
  );
};
