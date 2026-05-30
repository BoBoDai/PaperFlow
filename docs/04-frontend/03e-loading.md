# LoadingScreen — 加载页

`ui/src/components/LoadingSpinner.tsx` — 展示搜索/翻译进度。

## 两种模式

### 模式 1：简单加载（翻译中）

```tsx
<LoadingScreen
    message="翻译查询..."
    categories={[{ category: '中译英', status: 'fetching' }]}
/>
```

显示一个旋转字符 + 消息。

### 模式 2：多分类进度（快捷查询）

```tsx
<LoadingScreen
    message="机器人领域 · 查询中"
    categories={[
        { category: 'cs.RO', status: 'done', count: 3 },
        { category: 'cs.AI', status: 'fetching' },
        { category: 'cs.CV', status: 'pending' },
        { category: 'cs.LG', status: 'pending' },
    ]}
/>
```

## CategoryProgress

```typescript
interface CategoryProgress {
    category: string;                          // "cs.RO"
    status: 'pending' | 'fetching' | 'done' | 'error';
    count?: number;                            // 找到的论文数
}
```

## 旋转动画

使用 Braille spinner 字符集：

```typescript
const spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

useEffect(() => {
    const interval = setInterval(() => {
        setFrame(prev => (prev + 1) % spinner.length);
    }, 80);
    return () => clearInterval(interval);
}, []);
```

## 状态图标

| status | 图标 | 颜色 |
|--------|------|------|
| `done` | `✓` | green |
| `fetching` | spinner 字符 | yellow |
| `error` | `✗` | red |
| `pending` | `·` | dim |

## 渲染

```
  ⠋ 机器人领域 · 查询中

    ✓ cs.RO     3 篇
    ⠙ cs.AI     查询中...
    · cs.CV
    · cs.LG
```
