# PaperDetail — 论文详情组件

`ui/src/components/PaperDetail.tsx` — 论文详情展示，中英双语 + LLM 智能分析。

## 数据加载

```typescript
useEffect(() => {
    if (paper) { loadContent(); }
}, [paper]);

const loadContent = async () => {
    setIsLoading(true);
    // 并行请求：摘要 + 翻译
    const [sumResult, transResult] = await Promise.all([
        summarizePaper(paper, apiKey),   // LLM 智能分析
        translatePaper(paper.title, paper.summary || ''),  // 中译
    ]);
    setSummary(sumResult);
    if (transResult.success) setTranslation(transResult);
    setIsLoading(false);
};
```

**设计要点**：`Promise.all` 并行调用两个 API，比串行快一倍。

## 双语展示

```tsx
{/* 标题 — 英文 + 中文 */}
<Text bold>{paper.title}</Text>
{translation?.title_cn && (
    <Text color="cyan">{translation.title_cn}</Text>
)}

{/* 摘要 — 英文 + 中文 */}
<Text>{paper.summary}</Text>
{translation?.abstract_cn && (
    <Text color="cyan">{translation.abstract_cn}</Text>
)}
```

英文用默认色，中文用 cyan 色区分，视觉上一目了然。

## 摘要区域状态机

```
isLoading && !summary    → "正在生成..."
error                    → "失败: {error}"
!isLoading && !error && !summary → "暂无分析结果"
summary                  → 显示概述 / 分析 / 关键点
```

## 元信息展示

采用 label-value 布局（key dim 灰色，value 默认白色）：

```
作者  Zhang, Li, Wang, Chen
日期  2025-05-28
分类  cs.RO (Robotics)
来源  arXiv · 2505.12345
PDF   http://arxiv.org/pdf/...
```

## 来源标签

```typescript
const sourceText = paper.venue       // 优先用 venue（期刊/会议名）
    ? paper.venue
    : paper.source === 'arxiv'        // arXiv 论文
      ? `arXiv · ${paper.id}`
      : `${paper.source} · ${paper.id}`;
```

Semantic Scholar 论文带 venue（如 "ICRA 2025"），arXiv 论文显示 ID。
