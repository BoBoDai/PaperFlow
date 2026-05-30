import React, { useState, useEffect, useCallback } from 'react';
import { Text, Box } from 'ink';
import { useInput } from 'ink';

interface SearchPromptProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export const SearchPrompt: React.FC<SearchPromptProps> = ({
  value,
  onChange,
  onSubmit,
  onCancel,
}) => {
  const [cursorVisible, setCursorVisible] = useState(true);

  // Blinking cursor
  useEffect(() => {
    const interval = setInterval(() => {
      setCursorVisible(v => !v);
    }, 530);
    return () => clearInterval(interval);
  }, []);

  // Handle text input
  useInput((input, key) => {
    // Ctrl+C → cancel (check before key.ctrl guard below)
    if ((input === 'c' || input === '\x03') && key.ctrl) {
      onCancel();
      return;
    }

    // Escape → cancel (also handle raw escape char for some terminals)
    if (key.escape || input === '\x1b') {
      onCancel();
      return;
    }

    // Enter → submit
    if (key.return) {
      if (value.trim()) {
        onSubmit(value.trim());
      }
      return;
    }

    // Backspace
    if (key.backspace || key.delete) {
      onChange(value.slice(0, -1));
      return;
    }

    // Ignore other special keys
    if (key.tab || key.upArrow || key.downArrow || key.leftArrow || key.rightArrow) {
      return;
    }

    // Ignore control/meta (but allow through after handling Ctrl+C above)
    if (key.ctrl || key.meta) {
      return;
    }

    // Regular character input
    if (input && input.length > 0) {
      onChange(value + input);
    }
  });

  const cursor = cursorVisible ? '│' : ' ';

  return (
    <Box flexDirection="column" padding={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Text bold color="cyan">PaperFlow</Text>
        <Text dimColor> — 搜索论文</Text>
      </Box>

      {/* Input area */}
      <Box flexDirection="column" marginBottom={1}>
        <Text dimColor>输入关键词后按 Enter 搜索</Text>
        <Box marginTop={1}>
          <Text color="green" bold>{'>'} </Text>
          <Text>{value}</Text>
          <Text color="cyan">{cursor}</Text>
        </Box>
      </Box>

      {/* Suggestions */}
      <Box flexDirection="column" marginBottom={1}>
        <Text dimColor>示例  robot grasping  ·  multimodal learning  ·  diffusion model</Text>
        <Text dimColor>快捷  /robotics    /ai    /cv（输入后回车）</Text>
      </Box>

      {/* Hints */}
      <Box>
        <Text dimColor>Enter 搜索    Esc 返回</Text>
      </Box>
    </Box>
  );
};
