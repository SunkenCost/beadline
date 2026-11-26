<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline 图标" />
  <h1>Beadline</h1>
  <p><strong>面向 AI 协同开发的本地任务看板与依赖管理工具。</strong></p>
  <p>把分散在本地代码仓库里的任务变成清晰的可视化工作台，既方便人一眼看懂全局，又能与 <a href="https://github.com/steveyegge/beads">Beads (<code>bd</code>)</a> 及各类 AI Coding Agent 无缝联动。</p>
  <p>
    <a href="./README.md">English</a> · <strong>简体中文</strong>
  </p>
  <p>
    <img src="https://img.shields.io/badge/支持平台-Windows%20%7C%20macOS%20%7C%20Linux-555" alt="支持平台" />
    <img src="https://img.shields.io/badge/技术栈-Tauri%20v2%20%2B%20React%2019-D97757" alt="Tauri v2 + React 19" />
    <img src="https://img.shields.io/badge/开源协议-MIT-46B574" alt="MIT 协议" />
  </p>
</div>

---

## 这个项目用来做什么？

当你用 AI 编程助手（Claude Code、Codex、Cursor、T3 Code 等）同时推进多个功能或多个仓库时，很快会遇到几个真实痛点：

- **任务散落在聊天记录里** —— 对话拉长或开了好几个并行线程后，很难看清「哪些做完了、哪些正在跑、谁卡住了谁」。
- **云端项目管理工具（Jira / Linear / GitHub Projects）太重** —— 脱离本地代码仓库，AI 读写不便，人来回切网页也打断心流。
- **命令行任务工具（如 [Beads / `bd`](https://github.com/steveyegge/beads)）对 AI 友好，但人看得很累** —— 一旦拆出几十条主目标与子任务，光靠终端列表很难把握全局和依赖关系。

**Beadline 的作用，就是在本地给你提供一个「给人看全局、给 AI 接任务」的轻量桌面工作台：**

1. **理清全局进度**：连接一个或多个本地 Git 项目，用**看板、主目标进度、依赖图谱（DAG）**一眼看清所有任务状态与阻塞链。
2. **实时盯住 AI 干活**：AI Agent 在终端里创建、更新或关闭 `.beads/` 任务时，界面通过系统文件监听自动同步刷新，不用手动查命令。
3. **一键派发任务给 AI**：点开任意任务即可看到所属主目标背景、关联分支与 Commit，点一下**「复制 Agent 上下文」**就能直接贴给下一个 AI 会话开工。
4. **不装 `bd` 也能独立使用**：内置原生 Rust 存储引擎（约 `5 MB` 可执行文件、Windows 约 `90 MB` 内存），零外部依赖开箱即用；同时也 100% 双模兼容已有的 `bd` / Dolt 仓库。

## 核心功能

| | 你能直接用到什么 |
|---|---|
| **内置引擎 + `bd` 双模兼容** | 内置 Rust JSONL 引擎，零外部依赖即可创建、编辑、关单、管理依赖（含防环检测）与批注，`<2ms` 原子写入且 100% 兼容 `bd init --from-jsonl`；遇到 Dolt 仓库自动切双模同步，设置页支持一键校验下载官方 `bd`。 |
| **5 个高频视图** | **任务看板**（支持平铺 / 按主目标泳道分组）、**主目标**进度总览、可缩放拖拽的**依赖图谱（DAG）**、可多维筛选的**全部清单**，以及跨仓库的**全部进行中**视图。 |
| **多项目 × 模块分组** | 侧边栏一键切换本地仓库；按标签前缀自动拆出模块（默认 `mod:*`，可按项目改成 `area:` / `pkg:`），支持自定义中文别名与色点。 |
| **顺手的 AI 协作细节** | 一键**复制 Agent 上下文**；自动识别备注里的 Git 分支、Commit、GitHub `#编号`、本地 Worktree 目录及外部深链（`t3-thread://` / `vscode://` / `cursor://`）。 |
| **一眼看懂任务来龙去脉** | 详情抽屉顶部自动带出所属主目标背景与结案结论；看板支持一键折叠已完成的 AI 过程记录，减少满屏噪音。 |
| **主题与双语切换** | 内置 **Warm Clay**、**Cool Slate**、**Pure Mono** 三套主题（浅色 / 深色 / 跟随系统），支持强调色、圆角、排版密度调节、主题 JSON 导入导出与中英文实时切换。 |
| **零侵入、不污染仓库** | 项目列表、主题和模块别名全部存在系统用户目录，不会在任何 Git 仓库里生成多余配置文件。 |

## 快捷键

| 快捷键 | 功能 |
|---|---|
| `Ctrl+K` / `⌘K` | 打开命令面板 |
| `Ctrl+,` / `⌘,` | 打开设置 |
| `C` | 新建任务 |
| `/` | 聚焦搜索筛选框 |
| `0` – `4` | 切换视图（全部进行中 / 看板 / 主目标 / 依赖图谱 / 清单） |
| `Esc` | 关闭抽屉或弹窗 |

## 快速开始

**环境要求：** Node.js 20+、`pnpm`、[Rust 工具链](https://rustup.rs/)。（[`bd` CLI](https://github.com/steveyegge/beads) 为可选项 —— 仅在操作 Dolt 仓库或 GitHub 同步时需要，也可在应用设置内一键下载。）

```bash
git clone https://github.com/SunkenCost/beadline.git
cd beadline
pnpm install

# 开发模式启动
pnpm tauri dev

# 构建 Release 客户端 / 安装包
pnpm tauri build
```

## 工作原理

```text
本地 Git 仓库 (.beads/)
   │
   ├── 原生模式 ──▶ 内置 Rust JSONL 引擎（<2ms 原子写入、零外部依赖、100% 兼容 bd 导入）
   ├── Dolt 模式 ──▶ Rust 原生文件监听 + 直读 issues.jsonl + 串行 `bd` CLI 写队列（乐观更新 + 失败回滚）
   └── 应用配置 ──▶ 独立保存在系统用户目录（Windows 下位于 %APPDATA%\dev.beadline.desktop\）
```

- **不抢锁**：读取直接解析 `issues.jsonl` 并做内容指纹去重，多个 AI Agent 同时跑 `bd` 也不会冲突。
- **安全初始化与同步**：无侵入初始化仅写入 `.git/info/exclude`（不装 Git Hooks、不改 `AGENTS.md`），Dolt 仓库支持一键 `bd github sync`。

## 本地开发检查

```bash
pnpm typecheck && pnpm lint && pnpm test
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

更多底层设计细节可查看 [docs/design/architecture.md](./docs/design/architecture.md)。

## 开源协议

[MIT](./LICENSE)
