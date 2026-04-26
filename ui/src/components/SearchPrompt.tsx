import React from 'react';
import { Text, Box, Spacer } from 'ink';

interface SearchPromptProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}

export const SearchPrompt: React.FC<SearchPromptProps> = ({ value, onChange, onSubmit }) => {
  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">PaperFlow</Text>
        <Text> 搜索论文</Text>
      </Box>

      <Box borderStyle="round" padding={1} flexDirection="column">
        <Text>输入想了解的论文主题后按 Enter</Text>
        <Spacer />
        <Text dimColor>示例:</Text>
        <Text dimColor>  multimodal learning</Text>
        <Text dimColor>  CLIP, vision language</Text>
        <Text dimColor>  diffusion model</Text>
      </Box>

      <Spacer />

      <Box flexDirection="column" gap={1}>
        <Text dimColor>按 / 显示配置</Text>
        <Text dimColor>按 Esc 返回</Text>
      </Box>
    </Box>
  );
};
