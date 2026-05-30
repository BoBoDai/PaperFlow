# App — 搜索与快捷查询

## handleSearch — 关键词搜索

```typescript
const handleSearch = async (query: string): Promise<void> => {
    setMode('loading');
    setError(null);

    let term = query;
    const isChinese = /[一-龥]/.test(query);

    if (isChinese) {
        setMode('translating');
        const result = await translateQuery(query, apiKey);
        term = result.text;

        // 无 API Key → 显示错误
        if (!result.translated && !apiKey) {
            setError('未配置 API Key，中文翻译不可用...');
            setMode('error');
            return;
        }
    }

    const results = await searchArxiv(term, config.maxPapers);
    // 0 结果 → 错误模式（可立即重试）
    if (results.length === 0) {
        setError('未找到论文...');
        setMode('error');
    } else {
        setPapers(results);
        setMode('list');
    }
};
```

### 中文检测

正则 `/[一-龥]/` 匹配 CJK 统一表意文字。精确到 Unicode 区段，不误匹配日文假名和韩文。

### 0 结果处理

不进入列表模式，直接显示错误提示。用户按任意键立即进入搜索模式重试。

## handleQuickSearch — 快捷预设查询

```typescript
const handleQuickSearch = async (preset: Preset): Promise<void> => {
    setMode('loading');
    setLoadingMessage(`${preset.label} · 查询中`);

    // 动画进度
    setCategoryProgress(preset.categories.map(cat => ({
        category: cat, status: 'pending'
    })));

    let animTimer = setInterval(() => {
        setCategoryProgress(prev => {
            // 逐个标记分类为 fetching 状态
            if (step < prev.length) {
                const next = [...prev];
                next[step] = { ...next[step], status: 'fetching' };
                return next;
            }
            return prev;
        });
        step++;
    }, 1500);

    // 实际调用后端
    const results = await quickSearch(preset.id, config.maxPapers * 2);
    clearInterval(animTimer);

    // 更新分类计数
    setCategoryProgress(preset.categories.map(cat => ({
        category: cat,
        status: 'done',
        count: results.filter(p =>
            p.categories.some(c => c.startsWith(cat.split('.')[0]))
        ).length
    })));

    // 0 结果 → 降级尝试关键词搜索
    if (results.length === 0) {
        const fallback = await searchArxiv(preset.id, config.maxPapers);
        ...
    }
};
```

### 为什么用后端端点而非前端并行

最初版本前端对每个分类独立请求（streaming），但多个并行请求加剧 arXiv 限速。改为后端 `/api/quick-search` 单次请求，由后端控制延迟。前端只做动画效果。
