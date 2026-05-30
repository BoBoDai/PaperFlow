# Summary

[项目概述](01-overview.md)

# 架构设计

- [整体架构](02-architecture.md)
- [数据流](02-data-flow.md)

# 后端实现

- [入口与启动](03-backend/01-server.md)
- [核心类型](03-backend/02-core.md)
  - [Paper 模型](03-backend/02a-paper.md)
  - [错误处理](03-backend/02b-error.md)
  - [配置管理](03-backend/02c-config.md)
- [API 层](03-backend/03-api.md)
  - [路由注册](03-backend/03a-routes.md)
  - [搜索接口](03-backend/03b-search.md)
  - [翻译接口](03-backend/03c-translate.md)
  - [摘要与保存](03-backend/03d-summarize.md)
- [arXiv 客户端](03-backend/04-arxiv.md)
- [Semantic Scholar 客户端](03-backend/05-semantic-scholar.md)
- [LLM Provider 层](03-backend/06-llm.md)
  - [Trait 设计](03-backend/06a-trait.md)
  - [OpenAI Provider](03-backend/06b-openai.md)
  - [MiniMax Provider](03-backend/06c-minimax.md)
  - [Prompt 工程](03-backend/06d-prompt.md)
- [论文筛选与评分](03-backend/07-filter.md)
- [定时调度器](03-backend/08-scheduler.md)
- [存储层](03-backend/09-storage.md)

# 前端实现

- [入口与渲染](04-frontend/01-index.md)
- [App 主组件](04-frontend/02-app.md)
  - [状态管理](04-frontend/02a-state.md)
  - [键盘输入处理](04-frontend/02b-input.md)
  - [搜索与快捷查询](04-frontend/02c-search.md)
- [UI 组件](04-frontend/03-components.md)
  - [Welcome 欢迎页](04-frontend/03a-welcome.md)
  - [SearchPrompt 搜索输入](04-frontend/03b-search-prompt.md)
  - [PaperList 论文列表](04-frontend/03c-paper-list.md)
  - [PaperDetail 论文详情](04-frontend/03d-paper-detail.md)
  - [LoadingScreen 加载页](04-frontend/03e-loading.md)
  - [ConfigMenu 配置菜单](04-frontend/03f-config.md)
- [API 服务层](04-frontend/04-services.md)

# 配置与部署

- [配置文件说明](05-configuration.md)
