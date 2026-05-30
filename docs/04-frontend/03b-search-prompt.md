# SearchPrompt — 搜索输入组件

`ui/src/components/SearchPrompt.tsx` — PaperFlow 的核心交互组件，实现真正的终端文本输入。

## 为什么不用 HTML `<input>`

Ink 是 React for Terminal。终端没有浏览器 DOM，文字通过 `useInput` hook 逐按键捕获。SearchPrompt 用 React state 模拟了一个文本输入框。

## 逐字符输入

```typescript
useInput((input, key) => {
    // Ctrl+C → 取消
    if ((input === 'c' || input === '\x03') && key.ctrl) {
      onCancel(); return;
    }

    // Esc → 取消
    if (key.escape) { onCancel(); return; }

    // q → 取消（仅在输入为空时，有文字时 q 正常输入）
    if (input === 'q' && !value) { onCancel(); return; }

    // Enter → 提交
    if (key.return) {
      if (value.trim()) { onSubmit(value.trim()); }
      return;
    }

    // Backspace → 删除
    if (key.backspace || key.delete) {
      onChange(value.slice(0, -1));
      return;
    }

    // 方向键、Tab 等特殊键 → 忽略
    if (key.tab || key.upArrow || key.downArrow || ...) return;

    // Ctrl/Meta 组合键 → 忽略（防止 Ctrl+R 等输入 r 字符）
    if (key.ctrl || key.meta) return;

    // 普通字符 → 追加到输入
    if (input && input.length > 0) {
      onChange(value + input);
    }
});
```

## 闪烁光标

```typescript
const [cursorVisible, setCursorVisible] = useState(true);

useEffect(() => {
    const interval = setInterval(() => {
      setCursorVisible(v => !v);   // 每 530ms 反转
    }, 530);
    return () => clearInterval(interval);
}, []);
```

渲染：
```tsx
<Text color="green" bold>{'>'} </Text>
<Text>{value}</Text>
<Text color="cyan">{cursorVisible ? '│' : ' '}</Text>
```

效果：`> robot grasping│`（光标闪烁）。

## q 键的特殊处理

```typescript
if (input === 'q' && !value) { onCancel(); return; }
```

- 输入为空 → `q` 返回欢迎页
- 输入不为空 → `q` 正常追加（允许搜索 "quantum" 等词）

这解决了 IDE 终端拦截 Esc 的问题，同时不影响正常打字。

## Props 接口

```typescript
interface SearchPromptProps {
  value: string;          // 当前输入内容
  onChange: (v: string) => void;  // 更新输入
  onSubmit: (v: string) => void;  // 提交搜索
  onCancel: () => void;           // 取消返回
}
```

所有状态由父组件 App 管理，SearchPrompt 只管捕获按键和渲染。
