import React from 'react';
import { Text, Box, Spacer } from 'ink';

interface AppConfig {
  maxPapers: number;
  keywords: string[];
  voiceSpeed: number;
}

interface ConfigMenuProps {
  config: AppConfig;
  apiKey: string;
  onUpdate: (key: keyof AppConfig, value: AppConfig[keyof AppConfig]) => void;
  onClose: () => void;
}

export const ConfigMenu: React.FC<ConfigMenuProps> = ({ config, apiKey, onClose }) => {
  const configItems = [
    { key: 'maxPapers', label: '最大论文数', value: config.maxPapers },
    { key: 'keywords', label: '关键字', value: config.keywords.join(', ') || '(未设置)' },
    { key: 'voiceSpeed', label: '语速', value: config.voiceSpeed },
    { key: 'apiKey', label: 'API Key', value: apiKey ? '已配置' : '未配置' },
  ];

  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">配置菜单</Text>
      </Box>

      <Box borderStyle="round" padding={1} flexDirection="column" gap={1}>
        {configItems.map(({ key, label, value }) => (
          <Box key={key} justifyContent="space-between">
            <Text>{label}:</Text>
            <Text>{String(value)}</Text>
          </Box>
        ))}
      </Box>

      <Spacer />

      <Box flexDirection="column" gap={1}>
        <Text dimColor>配置说明:</Text>
        <Text dimColor>  /max &lt;数量&gt;  设置最大论文数</Text>
        <Text dimColor>  /keywords &lt;词1,词2&gt;  设置关键字</Text>
        <Text dimColor>  /voice &lt;速度&gt;  设置语速</Text>
      </Box>

      <Spacer />

      <Text dimColor>按 Esc 返回</Text>
    </Box>
  );
};
