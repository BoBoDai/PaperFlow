# 键盘输入处理

`App.tsx` 的 `useInput` 是全局键盘事件中枢。

## 架构设计

```
┌─────────────────────────────────────────────────┐
│                  useInput                        │
│                                                  │
│  Ctrl+C? ──→ 双击退出（最先处理）                  │
│       │                                          │
│  search 模式? ──→ 跳过（SearchPrompt 自己处理）    │
│       │                                          │
│  Esc / q? ──→ 返回上一步 / 退出                   │
│       │                                          │
│  模式分发 ──→ welcome / list / detail / error     │
└─────────────────────────────────────────────────┘
```

## 输入分离

搜索模式的文本输入由 `SearchPrompt` 组件自己的 `useInput` 处理，App 的 `useInput` 在搜索模式下直接 return：

```typescript
if (mode === 'search' && !showConfig) return;
```

这避免了两个 `useInput` 冲突——搜索时 SearchPrompt 独占输入，App 只处理配置弹出（`showConfig` 为 true）时的 Esc/q 关闭。

## 模式分发

```typescript
// 返回：q 或 Esc
if (isBack && !showConfig) {
  if (mode === 'welcome') { process.exit(0); }
  if (mode === 'search') { setMode('welcome'); }
  if (mode === 'list') { setMode('search'); }
  if (mode === 'detail') { setMode('list'); }
  if (mode === 'loading') { abortRef.current = true; setMode('welcome'); }
  ...
}

// Welcome: 单字母快捷查询
if (mode === 'welcome' && !key.ctrl) {
  if (input === 'r') handleQuickSearch(presets.robotics);
  ...
}

// List: 数字选论文
if (mode === 'list' && !key.ctrl) {
  const num = parseInt(input);
  if (num >= 1 && num <= papers.length) { ... }
}

// Detail: s/f/b
if (input === 's' && mode === 'detail') handleSpeak(...);
if (input === 'f' && mode === 'detail') handleSave(...);
if (input === 'b' && mode === 'detail') setMode('list');
```

## Ctrl 键保护

所有单字母快捷键都带 `!key.ctrl && !key.meta` 检查，防止 Ctrl+C / Ctrl+R 等组合键误触。

```typescript
if (input === 'r' && !key.ctrl && !key.meta) { ... }
if (input === 'c' && !key.ctrl && !key.meta) { ... }
// 等等
```

## 错误模式特殊处理

在 error 模式下，**任意键**（除 `/` 外）直接跳转到搜索模式：

```typescript
if (mode === 'error' && !showConfig) {
  if (input === '/') { setShowConfig(true); return; }
  setMode('search');  // 任意键 → 立即进入搜索
  setSearchQuery('');
  setError(null);
}
```
