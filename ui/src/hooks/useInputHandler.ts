import { useInput } from 'ink';

interface InputHandlerOptions {
  onEscape?: () => void;
  onSlash?: () => void;
  onEnter?: () => void;
  onUp?: () => void;
  onDown?: () => void;
  onKey?: (input: string, key: { escape?: boolean; return?: boolean; upArrow?: boolean; downArrow?: boolean }) => void;
  isActive?: boolean;
}

export const useInputHandler = (options: InputHandlerOptions): void => {
  const { onEscape, onSlash, onEnter, onUp, onDown, onKey, isActive = true } = options;

  useInput((input, key) => {
    if (!isActive) return;

    if (key.escape) {
      onEscape?.();
      return;
    }

    if (input === '/') {
      onSlash?.();
      return;
    }

    if (key.return) {
      onEnter?.();
      return;
    }

    if (key.upArrow) {
      onUp?.();
      return;
    }

    if (key.downArrow) {
      onDown?.();
      return;
    }

    onKey?.(input, key);
  }, { isActive });
};
