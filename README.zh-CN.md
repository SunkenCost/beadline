# Beadline

> **面向 AI Coding Agent 时代的轻量级跨平台 [Beads (`bd`)](https://github.com/steveyegge/beads) 原生桌面客户端。**

[English](./README.md)

Beadline 是一款开源、轻量、美观的跨平台（**Windows / macOS / Linux**）Beads 任务图桌面应用。基于 **Tauri v2 (Rust) + React 19 + Tailwind CSS v4 + Coss UI (`@base-ui/react`)** 构建，安装包约 10 MB、内存约 50 MB、秒开，且无需本地 Python 服务或端口监听。

## 核心特性

- **原生轻量体验**：无边框自绘标题栏（Windows 自绘三键 / macOS 交通灯适配）与窗口位置尺寸记忆；无冗余托盘驻留，无烦人弹窗打扰。
- **零轮询事件驱动引擎**：Rust 通过系统文件监听（`notify`）感知 `.beads/` 变动并毫秒级推送刷新；读取直读 `issues.jsonl` 避免与并发工作的 AI Agent 抢 Dolt 锁，写入走串行 `bd` CLI 队列，闲置 0% CPU。
- **克制的视觉与主题系统**：默认 **Warm Claude × Coss UI** 暖质感极简设计，内置 **Linear Slate / Pure Mono** 主题（支持浅色 / 深色 / 跟随系统），支持强调色、圆角、排版密度微调及 **Theme JSON 导入导出**。
- **多项目 × 大模块清晰分层**：一键切换本地仓库与「全部项目进行中」汇总视图；默认识别 `mod:*` 标签作为大模块（支持按项目自定义 `area:` / `pkg:` 前缀、显示别名与色点），**所有配置保存在系统用户目录，绝不污染业务仓库**。
- **Agent-Native 高频协作**：
  - 自动识别任务备注中的 **Git 分支 / Commit SHA / GitHub Issue / Worktree 路径 / 自定义深链（如 `t3-thread://`）**。
  - **一键复制「给 Agent 开工的结构化任务上下文」**（自动整合标题、编号、验收标准、前置阻塞与分支信息）。
  - 安全调用 `bd github sync` 与无侵入 `bd init`。
- **中英双语 & 键盘优先**：原生内置地道中英文切换、全局 `⌘K / Ctrl+K` 命令面板与单键快捷键（`C` 新建、`1–4` 切视图、`P` 切项目、`/` 筛选）。

## 设计文档与规范

- 完整架构与设计蓝图：[docs/design/architecture.md](./docs/design/architecture.md)
- 前端 / Rust 后端工程约束：[`.trellis/spec/`](./.trellis/spec/)

## 路线图

- [x] 项目架构设计、工程约束与 Trellis / Beads 协同环境初始化
- [ ] **Milestone 1**：Tauri v2 + React/Coss UI 骨架、Rust 混合事件引擎、无边框窗口、多项目/模块导航、主题与双语系统、4+1 核心视图
- [ ] **Milestone 2**：`⌘K` 命令面板、Theme JSON 导入导出、一键复制 Agent Prompt、GitHub 同步、完整设置页与 Windows/macOS 安装包构建

## 开源协议

[MIT](./LICENSE)
