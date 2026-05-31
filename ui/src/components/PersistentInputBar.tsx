import React, { useState, useEffect } from 'react';
import { Text, Box, useStdout } from 'ink';

interface PersistentInputBarProps {
  buffer: string;
  /** Breadcrumb showing current location, e.g. "首页 > 搜索结果" */
  location?: string;
}

export const PersistentInputBar: React.FC<PersistentInputBarProps> = ({ buffer, location }) => {
  const { stdout } = useStdout();
  const columns = (stdout?.columns ?? process.stdout.columns ?? 80) - 1;
  const divider = '─'.repeat(Math.max(columns, 0));

  const [cursorVisible, setCursorVisible] = useState(true);

  useEffect(() => {
    const interval = setInterval(() => setCursorVisible(v => !v), 530);
    return () => clearInterval(interval);
  }, []);

  const cursor = cursorVisible ? '│' : ' ';

  return (
    <Box flexDirection="column">
      <Text dimColor>{divider}</Text>
      {location && (
        <Box paddingLeft={1}>
          <Text dimColor>{location}</Text>
        </Box>
      )}
      <Box>
        <Text color="green" bold>❯ </Text>
        <Text>{buffer}</Text>
        <Text color="cyan">{cursor}</Text>
      </Box>
      <Text dimColor>{divider}</Text>
    </Box>
  );
};
