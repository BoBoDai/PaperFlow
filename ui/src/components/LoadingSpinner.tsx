import React, { useState, useEffect } from 'react';
import { Text, Box } from 'ink';

const spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

export interface CategoryProgress {
  /** Category label (e.g., "cs.RO") */
  category: string;
  /** Current status */
  status: 'pending' | 'fetching' | 'done' | 'error';
  /** Number of papers found (when done) */
  count?: number;
}

interface LoadingScreenProps {
  /** Main message shown at top */
  message: string;
  /** Per-category progress */
  categories: CategoryProgress[];
  /** Rotating search hints, shown one at a time */
  hints?: string[];
}

export const LoadingScreen: React.FC<LoadingScreenProps> = ({ message, categories, hints }) => {
  const [frame, setFrame] = useState(0);
  const [hintIndex, setHintIndex] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setFrame(prev => (prev + 1) % spinner.length);
    }, 80);
    return () => clearInterval(interval);
  }, []);

  // Rotate through hints every 4 seconds
  useEffect(() => {
    if (!hints || hints.length <= 1) return;
    const interval = setInterval(() => {
      setHintIndex(prev => (prev + 1) % hints.length);
    }, 4000);
    return () => clearInterval(interval);
  }, [hints]);

  const statusText = (cat: CategoryProgress): string => {
    switch (cat.status) {
      case 'done':
        return `${cat.count ?? 0} 篇`;
      case 'fetching':
        return '查询中...';
      case 'error':
        return '失败';
      case 'pending':
      default:
        return '';
    }
  };

  const statusColor = (cat: CategoryProgress): string | undefined => {
    switch (cat.status) {
      case 'done': return 'green';
      case 'fetching': return 'yellow';
      case 'error': return 'red';
      default: return undefined;
    }
  };

  const hasActiveFetch = categories.some(c => c.status === 'fetching');

  return (
    <Box flexDirection="column" padding={1}>
      {/* Main message */}
      <Box marginBottom={1}>
        {hasActiveFetch ? (
          <Text color="cyan">{spinner[frame]}</Text>
        ) : (
          <Text color="green">✓</Text>
        )}
        <Text> </Text>
        <Text bold>{message}</Text>
      </Box>

      {/* Search hint */}
      {hints && hints.length > 0 && (
        <Box marginBottom={1}>
          <Text>  </Text>
          <Text dimColor>💡 {hints[hintIndex]}</Text>
        </Box>
      )}

      {/* Category progress */}
      <Box flexDirection="column">
        {categories.map((cat) => (
          <Box key={cat.category}>
            <Text>  </Text>
            {/* Status icon */}
            {cat.status === 'fetching' ? (
              <Text color="yellow">{spinner[frame]} </Text>
            ) : cat.status === 'done' ? (
              <Text color="green">  ✓ </Text>
            ) : cat.status === 'error' ? (
              <Text color="red">  ✗ </Text>
            ) : (
              <Text dimColor>  · </Text>
            )}
            {/* Category name */}
            <Text dimColor={cat.status === 'pending'}>{cat.category}</Text>
            {/* Status / count */}
            {cat.status !== 'pending' && (
              <>
                <Text>  </Text>
                <Text color={statusColor(cat)} dimColor={cat.status === 'done'}>
                  {statusText(cat)}
                </Text>
              </>
            )}
          </Box>
        ))}
      </Box>
    </Box>
  );
};
