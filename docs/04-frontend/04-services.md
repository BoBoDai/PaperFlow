# API 服务层

前端通过两个 service 文件调用后端 API。

## arxiv.ts — 论文检索

```typescript
const API_BASE = 'http://localhost:8080';
```

| 函数 | 方法 | 端点 | 用途 |
|------|------|------|------|
| `searchArxiv(q, max)` | GET | `/api/search` | 关键词搜索 |
| `quickSearch(preset, max)` | GET | `/api/quick-search` | 快捷预设查询 |
| `searchByCategory(cat, max)` | GET | `/api/search?q=cat:{cat}` | 单分类查询 |
| `savePaper(paper)` | POST | `/api/papers/save` | 收藏论文 |
| `listPapers()` | GET | `/api/papers` | 列出已保存 |

### ArxivPaper 接口

```typescript
export interface ArxivPaper {
  id: string;              // arXiv ID
  title: string;
  summary: string;         // 原始摘要
  authors: string[];
  published: string;       // "2025-05-28"
  categories: string[];
  pdfUrl: string;
  source: string;          // "arxiv" | "semantic_scholar"
  venue: string | null;    // "ICRA 2025" | null
}
```

### 数据映射

后端 JSON → 前端接口的字段映射：

| 后端字段 | 前端字段 |
|---------|---------|
| `p.pdf_url` | `pdfUrl` |
| `p.summary` | `summary`（后端是 `abstract_text`）|

## minimax.ts — LLM 服务

| 函数 | 方法 | 端点 | 用途 |
|------|------|------|------|
| `translateQuery(q, key)` | POST | `/api/translate` | 中文→英文 |
| `translatePaper(title, abstract)` | POST | `/api/translate-paper` | 论文中译 |
| `summarizePaper(paper, key)` | POST | `/api/summarize` | LLM 智能分析 |
| `getConfig()` | GET | `/api/config` | 读取配置 |
| `updateConfig(c)` | POST | `/api/config` | 更新配置 |

### TranslateResult

```typescript
interface TranslateResult {
  text: string;          // 翻译后的文本
  translated: boolean;   // 是否成功（false 表示无 API Key 或失败）
}
```

`translated: false` 时前端显示警告提示用户配置 API Key。
