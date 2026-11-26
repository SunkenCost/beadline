<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline 图标" />
  <h1>Beadline</h1>
  <p>轻量、安静的 <a href="https://github.com/steveyegge/beads">Beads (<code>bd</code>)</a> 原生桌面客户端 —— 一站式管理多仓库任务看板、主目标与依赖图谱。</p>
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

用 Beads（`bd`）让 AI Agent 在仓库里管理任务很顺手，但一旦多个项目并行、任务拆到几十条，光靠终端命令看全局进度就很累。

**Beadline** 直接挂在你本地的 `.beads/` 目录上：**约 5 MB 可执行文件**、**Windows 下约 90 MB 内存**、文件变动实时同步，而且**绝不往你的业务仓库里写任何配置文件**。

## 核心功能

| | 你能直接用到什么 |
|---|---|
| **5 个高频视图** | **任务看板**（支持平铺 / 按主目标泳道分组）、**主目标**进度总览、可缩放拖拽的**依赖图谱（DAG）**、可多维筛选的**全部清单**，以及跨仓库的**全部进行中**视图。 |
| **多项目 × 模块分组** | 侧边栏一键切换本地仓库；按标签前缀自动拆出模块（默认 `mod:*`，可按项目改成 `area:` / `pkg:`），支持自定义中文别名与色点。 |
| **顺手的 AI 协作细节** | 一键**复制 Agent 上下文**（自动打包背景、验收标准、阻塞项与关联分支）；自动识别备注里的 Git 分支、Commit、GitHub `#编号`、本地 Worktree 目录及外部深链（`t3-thread://` / `vscode://` / `cursor://`）。 |
| **一眼看懂任务来龙去脉** | 详情抽屉顶部自动带出所属主目标背景与结案结论；看板支持一键折叠已完成的 AI 过程记录，不再满屏噪音。 |
| **主题与双语切换** | 内置 **Warm Clay**、**Cool Slate**、**Pure Mono** 三套主题（浅色 / 深色 / 跟随系统），支持强调色、圆角、排版密度调节、主题 JSON 导入导出与中英文实时切换。 |
| **零侵入、不污染仓库** | 项目列表、主题和模块别名全部存在系统用户目录，不会在任何 Git 仓库里生成多余文件。 |

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

**环境要求：** Node.js 20+、`pnpm`、[Rust 工具链](https://rustup.rs/)、[`bd` CLI](https://github.com/steveyegge/beads)。

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
   ├── 读取 ──▶ Rust 原生文件监听 + 直读 `.beads/issues.jsonl`（毫秒级同步，零轮询）
   ├── 写入 ──▶ 按仓库串行调用 `bd` CLI，前端 0ms 乐观更新，失败自动回滚
   └── 配置 ──▶ 独立保存在系统用户目录（Windows 下位于 %APPDATA%\dev.beadline.desktop\）
```

- **不抢锁**：读取直接解析 `issues.jsonl` 并做内容指纹去重，多个 AI Agent 同时跑 `bd` 也不会冲突。
- **安全初始化与同步**：支持无侵入初始化 Beads（写入 `.git/info/exclude`，不装 Git Hooks、不改 `AGENTS.md`）以及一键 `bd github sync`。

## 本地开发检查

```bash
pnpm typecheck && pnpm lint && pnpm test
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

更多底层设计细节可查看 [docs/design/architecture.md](./docs/design/architecture.md)。

## 开源协议

[MIT](./LICENSE)
