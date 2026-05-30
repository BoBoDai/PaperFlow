import React, { useState, useEffect } from 'react';
import { Text, Box } from 'ink';
import { useInput } from 'ink';

interface AppConfig {
  maxPapers: number;
  keywords: string[];
  voiceSpeed: number;
}

interface ConfigMenuProps {
  config: AppConfig;
  apiKey: string;
  onUpdate: (key: keyof AppConfig | 'apiKey', value: any) => void;
  onClose: () => void;
}

type Field = 'apiKey' | 'maxPapers' | 'voiceSpeed';

export const ConfigMenu: React.FC<ConfigMenuProps> = ({ config, apiKey, onUpdate, onClose }) => {
  const [activeField, setActiveField] = useState<Field>('apiKey');
  const [apiKeyValue, setApiKeyValue] = useState(apiKey);
  const [maxPapersValue, setMaxPapersValue] = useState(String(config.maxPapers));
  const [voiceSpeedValue, setVoiceSpeedValue] = useState(String(config.voiceSpeed));
  const [cursorVisible, setCursorVisible] = useState(true);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const interval = setInterval(() => setCursorVisible(v => !v), 530);
    return () => clearInterval(interval);
  }, []);

  const fields: Field[] = ['apiKey', 'maxPapers', 'voiceSpeed'];
  const currentValue = activeField === 'apiKey' ? apiKeyValue
    : activeField === 'maxPapers' ? maxPapersValue
    : voiceSpeedValue;

  const setCurrentValue = (val: string) => {
    if (activeField === 'apiKey') setApiKeyValue(val);
    else if (activeField === 'maxPapers') setMaxPapersValue(val);
    else setVoiceSpeedValue(val);
  };

  const saveField = () => {
    if (activeField === 'apiKey') {
      onUpdate('apiKey', apiKeyValue);
    } else if (activeField === 'maxPapers') {
      const n = parseInt(maxPapersValue);
      if (!isNaN(n) && n > 0) onUpdate('maxPapers', n);
    } else if (activeField === 'voiceSpeed') {
      const n = parseFloat(voiceSpeedValue);
      if (!isNaN(n) && n > 0) onUpdate('voiceSpeed', n);
    }
    setSaved(true);
    setTimeout(() => setSaved(false), 1200);
  };

  useInput((input, key) => {
    // Esc → close
    if (key.escape) {
      onClose();
      return;
    }

    // Tab / up/down → navigate fields
    if (key.tab || key.downArrow) {
      const idx = fields.indexOf(activeField);
      setActiveField(fields[(idx + 1) % fields.length]);
      setSaved(false);
      return;
    }
    if (key.upArrow) {
      const idx = fields.indexOf(activeField);
      setActiveField(fields[(idx - 1 + fields.length) % fields.length]);
      setSaved(false);
      return;
    }

    // Enter → save field
    if (key.return) {
      saveField();
      return;
    }

    // Backspace
    if (key.backspace || key.delete) {
      setCurrentValue(currentValue.slice(0, -1));
      setSaved(false);
      return;
    }

    // Regular characters
    if (input && input.length > 0 && !key.ctrl && !key.meta) {
      setCurrentValue(currentValue + input);
      setSaved(false);
    }
  });

  const cursor = cursorVisible ? '│' : ' ';

  const fieldLabel = (field: Field): string => {
    switch (field) {
      case 'apiKey': return 'API Key';
      case 'maxPapers': return '最大论文数';
      case 'voiceSpeed': return '语速';
    }
  };

  const fieldHint = (field: Field): string => {
    switch (field) {
      case 'apiKey': return 'MiniMax API Key';
      case 'maxPapers': return '每次搜索返回数量';
      case 'voiceSpeed': return 'TTS 语速 (1-10)';
    }
  };

  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">配置</Text>
        {saved ? <Text color="green">  已保存 ✓</Text> : null}
      </Box>

      <Box flexDirection="column" marginBottom={1}>
        {fields.map((field) => {
          const isActive = field === activeField;
          const val = field === 'apiKey' ? apiKeyValue
            : field === 'maxPapers' ? maxPapersValue
            : voiceSpeedValue;

          return (
            <Box key={field} flexDirection="column" marginBottom={1}>
              <Box>
                <Text color={isActive ? 'yellow' : undefined} bold={isActive}>
                  {isActive ? '> ' : '  '}{fieldLabel(field)}
                </Text>
                {!isActive ? (
                  <Text dimColor>
                    : {field === 'apiKey'
                      ? (val ? '●●●●●●●●' : '(未设置)')
                      : val}
                  </Text>
                ) : null}
              </Box>
              {isActive && (
                <Box flexDirection="column" marginLeft={2}>
                  <Box>
                    <Text dimColor>{fieldHint(field)}</Text>
                  </Box>
                  <Box>
                    <Text color="green">{'>'} </Text>
                    <Text>
                      {field === 'apiKey'
                        ? '●'.repeat(Math.min(val.length, 40))
                        : val}
                    </Text>
                    <Text color="cyan">{cursor}</Text>
                  </Box>
                </Box>
              )}
            </Box>
          );
        })}
      </Box>

      <Box flexDirection="column" marginBottom={1}>
        <Text>配置文件位置:</Text>
        <Text dimColor>  ~/.config/paperflow/config.toml</Text>
        <Text dimColor>  可直接编辑此文件，格式:</Text>
        <Text dimColor>    api_key = &quot;your-key&quot;</Text>
        <Text dimColor>    max_papers = 10</Text>
        <Text dimColor>    voice_speed = 5.0</Text>
      </Box>

      <Box>
        <Text dimColor>Tab 切换字段    Enter 保存    Esc 关闭</Text>
      </Box>
    </Box>
  );
};
