# 前端入口

`ui/src/index.tsx` — 渲染 Ink 应用并处理系统信号。

## 渲染配置

```typescript
const { waitUntilExit } = render(React.createElement(App), {
  exitOnCtrlC: false,   // 不自动退出，由代码处理 Ctrl+C
  patchConsole: false,  // 不拦截 console 输出
});
```

### exitOnCtrlC: false

默认情况下 Ink 在收到 SIGINT（Ctrl+C）时立即退出。设为 `false` 后，由应用自行处理——双击 Ctrl+C 退出。

### patchConsole: false

不拦截 `console.log`。如果设为 `true`，Ink 会把所有 console 输出重定向到 stderr 并通过 React 状态管理渲染。关掉后 console 输出直接到终端，用于调试。

## Ctrl+C 双击退出

```typescript
let sigintCount = 0;
let sigintTimer = null;

process.on('SIGINT', () => {
  sigintCount++;
  if (sigintCount >= 2) {
    unmount();
    process.exit(0);
  }
  console.log('\n  再次按 Ctrl+C 退出');
  sigintTimer = setTimeout(() => { sigintCount = 0; }, 3000);
});
```

注意：设置了 `exitOnCtrlC: false` 后，Ink 会拦截 SIGINT 信号，`process.on('SIGINT')` 可能收不到。因此 Ctrl+C 的实际处理逻辑在 App.tsx 的 `useInput` 中（`useInput` 的方式更可靠）。这段代码是双保险。
