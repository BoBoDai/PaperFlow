# 架构决策记录

记录 2026-10 这次重构中定下的方向，以及**被否决的方案和原因** ——
后者往往更重要，因为不写下来就会反复重新纠结。

---

## 1. 产品路线变了，技术栈得跟上

**旧路线**（2026-04 的规划）：全天候语音助理 —— 常驻 daemon、通勤收听、macOS 菜单栏。
Rust 是为这条路线选的，选得对：daemon、音频管道、单二进制，都是 Rust 的强项。

**实际在做的**：跨平台 GUI、v1 不做语音、LLM 分析论文。

这是两个不同的产品。技术栈停留在了旧路线上。

**结论**：保留 Rust 作为核心逻辑层，但界面全部交给 TypeScript。
唯一会推翻这个决定的条件是**回到常驻 daemon + 语音路线** —— 那时 Rust 的价值会重新变大。

---

## 2. 为什么保留 Rust 后端

不是沉没成本。理由：

- **Tauri 的内核就是 Rust** —— 后端和壳是同一个二进制，这是单文件安装的前提
- **界面无论如何都是 TS**（Tauri = Rust 后端 + Web 前端），所以「Rust GUI 生态弱」这个问题**不存在**
- 后端已有的领域逻辑（去重、打分、prompt 构造）是纯逻辑，本来就该有类型保障
- 将来若要回到 daemon 路线，`cpal` / 托盘 / 全局快捷键都是 Rust 一等公民

**代价（诚实记录）**：LLM 编排的生态确实不如 TS/Python。缓解方式是保持 LLM 逻辑薄，
真变重时**抽一块出去**（独立 crate → 独立进程），不是换一整套。

---

## 3. 被否决：GPUI（Zed 的 UI 框架）

Zed 是 100% 原生 Rust + 自研 GPU UI 框架 GPUI，不是 Electron、不是 Tauri、没有 WebView。
看起来很美，但：

- **没有官方 crates.io 发布** —— 只能依赖 Git commit，或社区镜像
  （`gpui-unofficial` / `gpui-pre` / `gpui-standalone` / `gpui-ce` 在互相打架）
- **2026 年初 Zed 给 GPUI 开发踩了刹车**，优先做商业化
- **许可证坑**：底层 `gpui` 是 Apache-2.0，但上层 UI 组件 crate 是 **GPL**
- 论文应用要的富文本渲染（KaTeX 公式、pdf.js、Markdown）在 Web 栈里是现成的，
  在 GPUI 里全部要自己写 —— 数学公式排版约等于实现一个 TeX 排版引擎

**关键澄清**：Zed 是「用 Rust 写 UI」，Tauri 是「用 Rust 写后端」。想保留 Rust，
正确的读法是前者不必要、后者可行。

---

## 4. 被否决：dora-rs 做节点编排

dora = Dataflow-Oriented **Robotic** Architecture，为机器人和实时 AI 管道造的中间件。是个好东西，但：

- **负载量级差六个数量级**：它优化「每秒几十次、每次几 MB」的零拷贝共享内存；
  我们是「每小时一次、每次几 KB」
- **架构形态冲突**：它是 CLI ↔ Coordinator ↔ Daemon ↔ Runtime 四层多进程，
  和「一个文件安装」直接对立
- **我们的 pipeline 不是图，是六行 `async fn`**

**根本原则**：编排框架的价值来自**跨越了什么边界**。

| 跨越了 | 需要 |
|---|---|
| 进程边界 | IPC 框架 |
| 机器边界 | 分布式框架 ← dora 在这里 |
| 语言边界 | IDL / 序列化框架 |
| 团队 / 时间边界 | 工作流引擎（Temporal、Airflow） |
| **什么都没跨** | **`async fn` 顺序调用** |

我们：同进程、同语言、同机器、一小时一次。**什么都没跨。**
真要重试/超时/限流，用 `tower` 的 middleware 组合子 —— 那是组合子，不是运行时。

---

## 5. 传输层：IPC，不是本地 HTTP

早期考虑过 Hono（或任何 HTTP 框架）起本地服务，**否决了**：

- **没有端口** 就没有「本机任意进程都能扫到你的服务」这个安全面 ——
  而 API key 就在那个进程内存里
- 没有 CORS 配置、没有鉴权 token、没有 HTTP 状态码语义
- **类型更强**：直接用共享 TS 类型标注 IPC 入参返回值，不需要额外描述序列化边界
- 这是主流做法：VS Code、Obsidian、Slack 都是 IPC

HTTP 唯一买到的是「能在普通浏览器标签页里开 UI」。而 electron-vite / Vite HMR
在应用窗口里已经给了热更新，所以这个损失很小。

**将来若要 Web 版**：因为 `usecases` 是传输无关的，加一层 HTTP 适配约 30 行，
不用重构。这个能力没有丢，只是没提前付钱。

**顺带一提**：arXiv API **没有 CORS 头**（实测 `HTTP/2 200` 但零个 `access-control-*`，
对比 Semantic Scholar 有 `access-control-allow-origin: *`）。所以渲染进程调不了 arXiv ——
这也是必须有非浏览器进程的硬理由之一。

---

## 6. 为什么 Tauri 而不是 Electron

结论依赖于**后端语言**：

> **Rust 核心 → Tauri；TS 核心 → Electron。**

Tauri 的体积优势（~10MB vs ~150MB）成立的前提是后端也是 Rust，和壳同语言。
后端一旦是 Node，就得给 Tauri 捆 sidecar 运行时，体积优势蒸发，还要吃
WebView 不一致的亏（Linux WebKitGTK 是重灾区）。

**选 Tauri 的额外收益**：
- 单文件安装包（macOS `.dmg` / Windows `.msi` / Linux `.AppImage`）
- 系统托盘、原生通知、全局快捷键、麦克风权限都是一等公民（为将来的语音路线留门）
- Rust 代码直接编译进二进制，不是 sidecar

**代价（诚实记录）**：
- Linux WebKitGTK 渲染差异 —— **必须尽早三平台实测**，这是唯一可能推翻方案的未知数
- Rust 编译时间（release 构建慢）
- Windows 老系统可能要装 WebView2 运行时

---

## 7. LLM 输出校验纪律

**所有 LLM 输出必须过结构校验才能返回。禁止任何形式的静默降级。**

旧实现里有一段典型的反面教材：

```rust
// Fallback: return the whole cleaned text (for title)
if field == "title" {
    text.lines().next()...   // 解析失败 → 把 LLM 原文第一行当标题返回
} else {
    String::new()            // 摘要解析失败 → 静默返回空字符串
}
```

**这是最糟的失败模式**：用户看到垃圾或空白，日志里一片安静。
而 `JSON.parse` 抓不到它 —— 它只抓语法错误。模型返回 `{"title_cn": "..."}` 时
JSON 完全合法，你读 `.title` 得到 `undefined`，静默失效。

所以 `paperflow_core::Error::InvalidLlmOutput` 是**刻意存在**的变体：
校验不过就报错，让调用方决定重试还是上报。

> 校验的**实现方式**（Zod / Valibot / 手写 type guard）可以再议；
> 这个**需求**不可议。

---

## 8. 数据模型要点

```sql
summaries(paper_id, model, prompt_version, short, detailed, key_points,
          created_at, PRIMARY KEY (paper_id, model, prompt_version))
          -- ↑ 模型和 prompt 版本进主键
```

**换了模型或改了 prompt，旧摘要自动失效**，不会被静默复用。
旧实现把 summary 内联在 paper 上，改完 prompt 之后旧结果会一直粘着，
而且你没法判断哪些是新的。
