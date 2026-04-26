import React from 'react';
import { Text, Box, Spacer } from 'ink';

interface WelcomeProps {
  onStart: () => void;
}

export const Welcome: React.FC<WelcomeProps> = () => {
  return (
    <Box flexDirection="column" padding={1}>
      <Box flexDirection="column" marginBottom={1}>
        <Text bold color="cyan">PaperFlow</Text>
        <Text dimColor>语音化学术助理</Text>
      </Box>

      <Box borderStyle="round" padding={1} flexDirection="column">
        <Text>输入想了解的论文主题</Text>
        <Text dimColor>例如: multimodal learning, CLIP, diffusion model</Text>
      </Box>

      <Spacer />

      <Box flexDirection="column" gap={1}>
        <Text dimColor>快捷键:</Text>
        <Text dimColor>  /     显示配置菜单</Text>
        <Text dimColor>  esc   返回</Text>
        <Text dimColor>  enter 确认</Text>
      </Box>

      <Spacer />

      <Text color="green" bold>
        按 Enter 开始 {'>'}
      </Text>
    </Box>
  );
};
