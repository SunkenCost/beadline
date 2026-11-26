<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline 图标" />
  <h1>Beadline</h1>
  <p><strong>面向多 AI 并行开发的本地战情室、任务依赖看板与极速 CLI / MCP 引擎。</strong></p>
  <p>当你同时驱动 Claude Code、Codex、Cursor、Antigravity、T3 Code 等多个 AI 编程助手时，让你一眼看清谁在做什么、有没有撞车、谁卡在等你确认 —— 零外部数据库进程。</p>
  <p>
    <a href="./README.md">English</a> · <strong>简体中文</strong>
  </p>
  <p>
    <img src="https://img.shields.io/badge/支持平台-Windows%20%7C%20macOS%20%7C%20Linux-555" alt="支持平台" />
    <img src="https://img.shields.io/badge/技术栈-Rust%20%2B%20Tauri%20v2%20%2B%20React%2019-D97757" alt="Rust + Tauri v2 + React 19" />
    <img src="https://img.shields.io/badge/开源协议-MIT-46B574" alt="MIT 协议" />
  </p>
</div>

---

## 这个项目用来做什么？

当你同时开多个线程或多个 Git Worktree 跑 AI Coding Agent 时，最容易让人“心里发虚”的是四件事：

- **不知道两个 AI 有没有默默改到同一批文件** —— 直到合并时才炸出一堆冲突。
- **不知道哪个 AI 正在干活、哪个停滞了、哪个其实在等你确认** —— 得挨个窗口翻对话记录。
- **任务散落、阻塞链不清** —— 看不出先解哪一张根任务能一口气放行后面所有依赖。
- **外部 CLI 数据库过重** —— 传统 `bd`（约 `150 MB`）冷启动慢、易抢锁，多 Worktree 切分支还容易出现多份任务状态。

**Beadline 把整套底层与界面统一成一个轻量 Rust 工作区（`beadline-core` + `bl` CLI/MCP + `beadline` 桌面端）：**

1. **战情室（Mission Control）**：首屏 4 块在 3 秒内消除失控感 —— **等我处理**（明确等待确认 / 同改文件 / 进展停滞 / 遥测失联）、**谁在做什么 + 未归属编辑**、**证据分级改动重叠**、**先解哪一个根阻塞**。
2. **跨 Harness 统一引擎（`bl` + `bl mcp`）**：约 `1.6 MB` 的纯 Rust 独立二进制，仅暴露 **5 个高杠杆 MCP 工具**（`bl_ready` / `bl_show` / `bl_claim` / `bl_pulse` / `bl_finish`）并镜像完整 CLI，兼容 Claude Code、Codex CLI、Cursor、Antigravity、T3 Code 及任意终端 Agent。
3. **跨 Worktree 共享唯一真相，心跳按会话分文件**：任务权威库放在 `<git-common-dir>/beadline/issues.jsonl`（全 worktree 共享、OS 文件锁保护、安全原子替换、零污染 `git status`）；在线租约按会话独立存放在 `runtime/pulses/<session-id>.json`，多 Agent 并行续租互不争锁。
4. **兼容 Beads（`bd`）交换**：提供 `bl import --from` / `bl export --format bd-v1`（带基线防漂移校验），同时继续无缝读取已有的 `.beads/` / Dolt 仓库。

## 核心功能

| | 你能直接用到什么 |
|---|---|
| **战情室与证据分级冲突雷达** | 三级重叠预警（**两边工作区实际同改路径** > **实际改动命中对方声明 scope** > **仅声明 scope 重叠**）；基于 `git --no-optional-locks status` 的无锁观测；诚实区分已认领会话与未归属编辑；自动排序可立即开工的根阻塞。 |
| **极速 `bl` CLI & `bl mcp`** | 约 `1.6 MB` 单文件，零 GUI 依赖。5 个精简 MCP 工具支持 `expectedRev` 乐观并发、`operationId` 幂等关单、结构化交付证据（`proof`）与自动返回新解锁任务；设置页内置各主流 AI 工具绝对路径配置一键复制与 `doctor` 体检。 |
| **6 个高频视图** | **战情室**、**任务看板**（每张卡最多展示一条最高优先级实时状态提示）、**主目标**、交互式**依赖图谱（DAG）**、**全部清单**以及跨项目**全部进行中**。 |
| **独立遥测通道与低内存占用** | 动态会话心跳与 Git 雷达走独立 `RadarSnapshot` 通道，不触发整板重渲染；静止时零 Git 轮询，桌面端约 `5.4 MB`。 |
| **AI 交接与上下文** | 抽屉内置**复制 Agent 交接单**（自动带出为什么做、验收标准、声明写范围及 `bl_claim / bl_pulse / bl_finish` 协议），自动提取分支、Commit、PR、Worktree 与深链。 |
| **克制主题与中英双语** | 内置 **Warm Clay**、**Cool Slate**、**Pure Mono** 三套预设（浅色 / 深色 / 跟随系统），支持主题 JSON 导入导出与中英文实时切换。 |

## 快捷键

| 快捷键 | 功能 |
|---|---|
| `M` | 切换到战情室 |
| `0` – `4` | 切换视图（全部进行中 / 看板 / 主目标 / 依赖图谱 / 清单） |
| `Ctrl+K` / `⌘K` | 打开命令面板 |
| `Ctrl+,` / `⌘,` | 打开设置（外观主题 / 引擎 / AI 接入配置） |
| `C` | 新建任务 |
| `/` | 聚焦搜索筛选框 |
| `Esc` | 关闭抽屉或弹窗 |

## 快速开始

**环境要求：** Node.js 20+、`pnpm`、[Rust 1.89+](https://rustup.rs/)。

```bash
git clone https://github.com/SunkenCost/beadline.git
cd beadline
pnpm install

# 构建独立 bl CLI + MCP 引擎（输出到 target/release/bl[.exe]）
cargo build --release -p beadline-cli

# 开发模式启动桌面端
pnpm tauri dev

# 构建 Release 桌面端
pnpm tauri build
```

### 一键接入你的 AI 工具

打开 Beadline 桌面端 **设置 → AI 接入 (MCP / CLI)**，可直接复制带 `bl` 绝对路径的配置到 **Claude Code**、**Codex CLI**、**Cursor**、**Antigravity** 或 **T3 Code / 通用 CLI**，也可以在终端直接使用：

```bash
bl init
bl doctor
bl ready
bl claim <issue-id> --summary "实现认证模块" --scope "src/auth/**"
bl finish <issue-id> --summary "单元测试全部通过"
```

## 工作区结构

```text
Cargo Workspace
├── crates/beadline-core   # RepoLocator、OS 锁原子 JSONL 引擎、分会话租约、Git Worktree 冲突雷达
├── crates/beadline-cli    # 独立 bl CLI 与 bl mcp stdio JSON-RPC 2.0 服务端（约 1.6 MB）
└── src-tauri              # Tauri v2 桌面端 + 双通道文件监听（持久化看板 vs 动态雷达）
```

## 本地开发检查

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## 开源协议

[MIT](./LICENSE)
