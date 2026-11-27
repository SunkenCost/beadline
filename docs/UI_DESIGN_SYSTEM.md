# Calm Workbench UI Design System (AI 规范与复用模板)

> 沉淀自 **Beadline**（Tauri v2 + React 19 + Tailwind CSS v4 + `@base-ui/react`）的桌面 / Web 专业工作台 UI 体系。
> 本文件既可直接放入新项目的 `docs/UI_DESIGN_SYSTEM.md`，也可把 **第 1 节「AI 执行契约」** 直接粘贴进 `AGENTS.md` / `CLAUDE.md` / `GEMINI.md`，约束 AI 稳定产出克制、耐看、高频顺手的专业工具界面。

---

## 1. AI 执行契约（可直接粘贴到项目的 `AGENTS.md`）

```markdown
## UI & Interaction Rules (Calm Workbench System)

1. **禁止硬编码颜色**：组件中一律使用语义 CSS 变量（`var(--bg)`, `var(--surface)`, `var(--fg)`, `var(--border)`, `var(--accent)` 等），禁止直接写 `#fff`、`bg-white`、`text-gray-500` 或渐变花哨背景。
2. **用底色微差和发丝边框分层，不用重阴影**：背景严格按 `--bg` → `--sidebar` → `--surface` → `--surface-subtle` 分层；边框使用极低透明度发丝线（`--border`）；阴影仅用于浮层（Popover / Dialog / Drawer）与卡片轻悬停。
3. **极简组件栈**：仅使用 `Tailwind CSS v4` + `@base-ui/react`（Headless 交互原语）+ `lucide-react`（线性图标），不引入 AntD / MUI 等重视觉样式库。
4. **信息密度与单行顶栏**：主区顶栏保持单行（≤ 4 组主控件：标题/筛选胶囊 + 搜索 + `显示` Popover + 主按钮）；次级视图开关统一收进 `显示` Popover，禁止在顶栏平铺 6 个以上零碎按钮。
5. **多条目默认「左分组列表 + 右常驻详情分栏」**：当主业务对象可能超过 15 条时，默认采用 Master-Detail 分栏（默认聚焦「未完成 / 活跃」项、隐藏空分组、支持 `↑/↓` 或 `J/K` 毫秒级切换预览）；窄屏（`<1024px`）自动降级为滑出抽屉并复用同一份 Detail 组件。看板/图表作为 1-click 可选视图。
6. **内容与高频动作置顶**：详情面板严格按 `标题 → 一键状态/标签切换条 → 正文与核心上下文 → 关联项 → 低频表单属性` 排序，绝不把一大块表单输入框堆在正文上方。
7. **防挤压与防重叠 CSS 铁律**：
   - 所有按钮、ID、短状态标签、分段选项卡、表头必须加 `shrink-0 whitespace-nowrap`。
   - 任何使用 `truncate` 的 Flex 子项，其父链必须有 `min-w-0`。
   - 多列横向 Grid 必须使用 `grid-cols-[repeat(N,minmax(220px,1fr))]` + 子列 `min-w-0`，**严禁** `grid-cols-N` 搭配子元素固定 `min-w-[...]`（会导致列内容溢出盖住相邻列）。
   - 卡片容器必须带 `min-w-0 overflow-hidden`，长标题带 `break-words`。
8. **高频操作缩到 1 步 + 全键盘流**：列表行/卡片常态降噪，悬停在右侧浮出 1–2 个高频一键按钮；提供行内回车速记框；标配 `Ctrl+K` 命令面板、`/` 搜素、`↑↓` 切行、`Esc` 逐层关闭、`Ctrl+Enter` 提交。
```

---

## 2. 极简技术栈清单

```json
{
  "dependencies": {
    "@base-ui/react": "^1.9.0",
    "lucide-react": "^1.80.0",
    "react": "^19.3.1",
    "react-dom": "^19.3.1",
    "zustand": "^5.1.0"
  },
  "devDependencies": {
    "@biomejs/biome": "^2.0.0",
    "@tailwindcss/vite": "^4.3.5",
    "tailwindcss": "^4.3.5",
    "typescript": "^5.9.0",
    "vite": "^8.3.8"
  }
}
```

**选型原则**：
- **`@base-ui/react`**：只接管焦点陷阱、键盘导航、Portal 定位与 `data-[starting-style]` / `data-[ending-style]` 过渡态，零默认视觉污染。
- **`Tailwind CSS v4`**：零配置文件，直接用 `@import "tailwindcss";` + `@import "./tokens.css";`。
- **`Zustand` 关注点拆店**：把「纯 UI 态（视图/筛选/弹窗/选中项）」与「业务数据态」「高频实时态」拆成独立 store，避免高频刷新导致整棵树重渲染。

---

## 3. 语义化 Design Token 模板（可直接复制使用）

### 3.1 `src/styles/tokens.css`

提供三套经实机打磨的克制预设（默认旗舰 **Warm Clay 暖陶纸色**，比纯白纯黑更护眼耐看；另有 **Cool Slate 冷灰蓝** 与 **Pure Mono 极简黑白**），通过 `<html data-preset data-theme data-radius data-density>` 无刷新切换：

```css
:root {
  --font-sans:
    "Inter", -apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans SC",
    "Microsoft YaHei UI", "Segoe UI", sans-serif;
  --font-mono:
    "JetBrains Mono", "Cascadia Code", "SFMono-Regular", ui-monospace, Consolas, monospace;

  --ease: cubic-bezier(0.2, 0.8, 0.2, 1);

  /* 圆角体系 (data-radius="default") */
  --radius-xs: 4px;
  --radius-sm: 6px;
  --radius-md: 8px;
  --radius-lg: 10px;

  /* 排版密度 (data-density="comfortable") */
  --row-height: 30px;
  --card-pad-y: 11px;
  --card-pad-x: 12px;
  --lane-gap: 6px;
}

:root[data-radius="sharp"] {
  --radius-xs: 2px;
  --radius-sm: 3px;
  --radius-md: 4px;
  --radius-lg: 6px;
}

:root[data-radius="soft"] {
  --radius-xs: 6px;
  --radius-sm: 8px;
  --radius-md: 11px;
  --radius-lg: 14px;
}

:root[data-density="compact"] {
  --row-height: 26px;
  --card-pad-y: 8px;
  --card-pad-x: 10px;
  --lane-gap: 4px;
}

/* ---------- Preset 1: Warm Clay (默认旗舰 · 暖陶纸色) ---------- */
:root,
:root[data-preset="warm-clay"][data-theme="light"] {
  --bg: #faf9f6;
  --sidebar: #f4f3ee;
  --surface: #ffffff;
  --surface-subtle: #f6f5f1;
  --surface-hover: #efece6;
  --surface-active: #e7e4dc;

  --fg: #181817;
  --fg-2: #5c5b56;
  --fg-3: #8f8d86;

  --border: rgba(24, 24, 23, 0.085);
  --border-strong: rgba(24, 24, 23, 0.16);
  --border-subtle: rgba(24, 24, 23, 0.05);

  --primary: #181817;
  --primary-fg: #faf9f6;
  --primary-hover: #2e2e2b;

  --accent: #c96442;
  --accent-soft: rgba(201, 100, 66, 0.1);
  --accent-border: rgba(201, 100, 66, 0.28);

  --success: #16803c;
  --success-soft: rgba(22, 128, 60, 0.09);
  --warning: #b45309;
  --danger: #c53030;
  --danger-soft: rgba(197, 48, 48, 0.08);
  --info: #2563eb;

  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-sm: 0 2px 8px -2px rgba(0, 0, 0, 0.07), 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-lg: 0 16px 40px -12px rgba(0, 0, 0, 0.18), 0 2px 6px rgba(0, 0, 0, 0.05);
}

:root[data-preset="warm-clay"][data-theme="dark"] {
  --bg: #121211;
  --sidebar: #151514;
  --surface: #1a1a18;
  --surface-subtle: #161615;
  --surface-hover: #22221f;
  --surface-active: #292926;

  --fg: #ecebe6;
  --fg-2: #9b9992;
  --fg-3: #686661;

  --border: rgba(255, 255, 255, 0.075);
  --border-strong: rgba(255, 255, 255, 0.15);
  --border-subtle: rgba(255, 255, 255, 0.045);

  --primary: #ecebe6;
  --primary-fg: #141413;
  --primary-hover: #dcdad2;

  --accent: #d97757;
  --accent-soft: rgba(217, 119, 87, 0.12);
  --accent-border: rgba(217, 119, 87, 0.3);

  --success: #46b574;
  --success-soft: rgba(70, 181, 116, 0.11);
  --warning: #e0a84b;
  --danger: #e06c65;
  --danger-soft: rgba(224, 108, 101, 0.11);
  --info: #6ea1e8;

  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.28);
  --shadow-sm: 0 4px 14px -4px rgba(0, 0, 0, 0.48), 0 1px 0 rgba(255, 255, 255, 0.03) inset;
  --shadow-lg: 0 20px 48px -12px rgba(0, 0, 0, 0.72), 0 0 0 1px rgba(255, 255, 255, 0.06);
}

/* ---------- Preset 2: Cool Slate (冷灰蓝 · Linear 风格) ---------- */
:root[data-preset="cool-slate"][data-theme="light"] {
  --bg: #fcfcfd;
  --sidebar: #f5f6f8;
  --surface: #ffffff;
  --surface-subtle: #f7f8fa;
  --surface-hover: #eef0f4;
  --surface-active: #e5e8ef;
  --fg: #111318;
  --fg-2: #555b69;
  --fg-3: #8a909e;
  --border: rgba(17, 19, 24, 0.085);
  --border-strong: rgba(17, 19, 24, 0.16);
  --border-subtle: rgba(17, 19, 24, 0.05);
  --primary: #111318;
  --primary-fg: #ffffff;
  --primary-hover: #262a33;
  --accent: #5e6ad2;
  --accent-soft: rgba(94, 106, 210, 0.1);
  --accent-border: rgba(94, 106, 210, 0.3);
  --success: #16803c;
  --success-soft: rgba(22, 128, 60, 0.09);
  --warning: #b45309;
  --danger: #d13438;
  --danger-soft: rgba(209, 52, 56, 0.09);
  --info: #2563eb;
  --shadow-xs: 0 1px 2px rgba(15, 23, 42, 0.04);
  --shadow-sm: 0 2px 8px -2px rgba(15, 23, 42, 0.07), 0 1px 2px rgba(15, 23, 42, 0.04);
  --shadow-lg: 0 16px 40px -12px rgba(15, 23, 42, 0.18), 0 2px 6px rgba(15, 23, 42, 0.05);
}

:root[data-preset="cool-slate"][data-theme="dark"] {
  --bg: #0e1013;
  --sidebar: #121418;
  --surface: #171a20;
  --surface-subtle: #14161b;
  --surface-hover: #1f232b;
  --surface-active: #262b35;
  --fg: #eceff4;
  --fg-2: #9aa2b1;
  --fg-3: #636b7a;
  --border: rgba(255, 255, 255, 0.075);
  --border-strong: rgba(255, 255, 255, 0.15);
  --border-subtle: rgba(255, 255, 255, 0.045);
  --primary: #eceff4;
  --primary-fg: #0e1013;
  --primary-hover: #d8dee9;
  --accent: #7a86e8;
  --accent-soft: rgba(122, 134, 232, 0.14);
  --accent-border: rgba(122, 134, 232, 0.34);
  --success: #46b574;
  --success-soft: rgba(70, 181, 116, 0.11);
  --warning: #e0a84b;
  --danger: #e06c75;
  --danger-soft: rgba(224, 108, 117, 0.12);
  --info: #6ea1e8;
  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.32);
  --shadow-sm: 0 4px 14px -4px rgba(0, 0, 0, 0.54);
  --shadow-lg: 0 20px 48px -12px rgba(0, 0, 0, 0.78), 0 0 0 1px rgba(255, 255, 255, 0.06);
}

/* ---------- Preset 3: Pure Mono (极简黑白) ---------- */
:root[data-preset="pure-mono"][data-theme="light"] {
  --bg: #fafafa;
  --sidebar: #f3f3f3;
  --surface: #ffffff;
  --surface-subtle: #f6f6f6;
  --surface-hover: #ececec;
  --surface-active: #e2e2e2;
  --fg: #0a0a0a;
  --fg-2: #525252;
  --fg-3: #8c8c8c;
  --border: rgba(10, 10, 10, 0.09);
  --border-strong: rgba(10, 10, 10, 0.18);
  --border-subtle: rgba(10, 10, 10, 0.05);
  --primary: #0a0a0a;
  --primary-fg: #fafafa;
  --primary-hover: #262626;
  --accent: #171717;
  --accent-soft: rgba(23, 23, 23, 0.08);
  --accent-border: rgba(23, 23, 23, 0.24);
  --success: #16803c;
  --success-soft: rgba(22, 128, 60, 0.09);
  --warning: #b45309;
  --danger: #c53030;
  --danger-soft: rgba(197, 48, 48, 0.08);
  --info: #2563eb;
  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-sm: 0 2px 8px -2px rgba(0, 0, 0, 0.07);
  --shadow-lg: 0 16px 40px -12px rgba(0, 0, 0, 0.16);
}

:root[data-preset="pure-mono"][data-theme="dark"] {
  --bg: #0b0b0b;
  --sidebar: #101010;
  --surface: #161616;
  --surface-subtle: #121212;
  --surface-hover: #1e1e1e;
  --surface-active: #262626;
  --fg: #f4f4f4;
  --fg-2: #9e9e9e;
  --fg-3: #666666;
  --border: rgba(255, 255, 255, 0.08);
  --border-strong: rgba(255, 255, 255, 0.16);
  --border-subtle: rgba(255, 255, 255, 0.045);
  --primary: #f4f4f4;
  --primary-fg: #0b0b0b;
  --primary-hover: #dddddd;
  --accent: #e5e5e5;
  --accent-soft: rgba(229, 229, 229, 0.1);
  --accent-border: rgba(229, 229, 229, 0.28);
  --success: #46b574;
  --success-soft: rgba(70, 181, 116, 0.11);
  --warning: #e0a84b;
  --danger: #e06c65;
  --danger-soft: rgba(224, 108, 101, 0.11);
  --info: #6ea1e8;
  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.35);
  --shadow-sm: 0 4px 14px -4px rgba(0, 0, 0, 0.58);
  --shadow-lg: 0 20px 48px -12px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(255, 255, 255, 0.06);
}
```

### 3.2 `src/styles/globals.css`

```css
@import "tailwindcss";
@import "./tokens.css";

@layer base {
  * {
    box-sizing: border-box;
    border-color: var(--border);
  }

  html,
  body,
  #root {
    height: 100%;
    width: 100%;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  body {
    font-family: var(--font-sans);
    font-size: 13px;
    line-height: 1.5;
    letter-spacing: -0.01em;
    color: var(--fg);
    background-color: var(--bg);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    user-select: none; /* 桌面端质感：正文/输入框单独开启 select-text */
  }

  input,
  textarea,
  select {
    user-select: text;
  }

  :focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* 6px 极细半透明滚动条 */
  ::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  ::-webkit-scrollbar-track {
    background: transparent;
  }
  ::-webkit-scrollbar-thumb {
    background: var(--border-strong);
    border-radius: 3px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background: var(--fg-3);
  }
}

.font-mono-tabular {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
```

---

## 4. 尺寸、字号与层级标尺

| 位置 | 规范值 | 说明 |
|---|---|---|
| 窗口标题栏 (`Titlebar`) | `h-10` (`40px`) | 左侧品牌 + 折叠侧栏；中间纯拖拽留白；右侧命令面板 / 语言 / 主题 / 设置 / 窗口按钮 |
| 侧边栏 (`Sidebar`) | `w-[208px]`（折叠 `w-[52px]`） | 一级视图 ≤ 4–5 个；空区块（如进行中为 0）自动隐藏不占位 |
| 主区顶栏 (`Topbar`) | `min-h-11` (`44px`), `px-4` | 单行布局：左侧标题 + 当前筛选胶囊，右侧搜索 + `显示` Popover + 主按钮 |
| 标准控件 / 按钮 | `h-7` (`28px`), `text-[12px]` | 所有按钮与胶囊固定 `shrink-0 whitespace-nowrap` |
| 紧凑控件 / 行内操作 | `h-6` (`24px`), `text-[11.5px]` | 用于卡片悬停动作、筛选胶囊、Popover 内部切换 |
| 列表行 / 任务标题 | `text-[13px]`, `leading-[1.45]` | 正文 `var(--fg)`；次级信息 `var(--fg-2)`；编号/时间/计数 `var(--fg-3)` + `.font-mono-tabular` |
| 右侧常驻详情分栏 | `w-[390px] xl:w-[450px]` | `>=1024px` 常驻右侧无遮罩；`<1024px` 自动降级为滑出抽屉 |
| 过渡动效 | `100ms–150ms` | 仅做 `opacity / colors / border / 0.98→1 scale`，不做弹跳或花哨缓动 |

---

## 5. 超薄 UI 原语层骨架（`src/components/ui/`）

项目只维护极薄的一层基础件（每个文件 30–70 行），业务代码统一调用这组原语：

### `Button.tsx`（仅 4 种语义变体）

```tsx
import type { ButtonHTMLAttributes } from "react";

type ButtonVariant = "primary" | "outline" | "ghost" | "accent";
type ButtonSize = "sm" | "md" | "icon";

const VARIANT: Record<ButtonVariant, string> = {
  primary: "bg-[var(--primary)] text-[var(--primary-fg)] hover:bg-[var(--primary-hover)] font-medium",
  outline: "bg-[var(--surface)] border border-[var(--border)] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] font-medium",
  ghost: "bg-transparent text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] font-medium",
  accent: "bg-[var(--accent)] text-white hover:brightness-105 font-medium",
};

const SIZE: Record<ButtonSize, string> = {
  sm: "h-6 px-2 text-[11.5px] gap-1 rounded-[var(--radius-xs)]",
  md: "h-7 px-2.5 text-[12px] gap-1.5 rounded-[var(--radius-sm)]",
  icon: "h-7 w-7 p-0 justify-center rounded-[var(--radius-sm)]",
};

export function Button({
  variant = "ghost",
  size = "md",
  className = "",
  type = "button",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant; size?: ButtonSize }) {
  return (
    <button
      type={type}
      className={`inline-flex shrink-0 items-center whitespace-nowrap transition-colors duration-100 cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...props}
    />
  );
}
```

### `Popover.tsx`（`@base-ui/react` 150ms 轻柔过渡）

```tsx
import { Popover as BasePopover } from "@base-ui/react/popover";
import type { ReactElement, ReactNode } from "react";

export function Popover({
  trigger,
  children,
  align = "end",
  sideOffset = 6,
  className = "",
}: {
  trigger: ReactElement;
  children: ReactNode;
  align?: "start" | "center" | "end";
  sideOffset?: number;
  className?: string;
}) {
  return (
    <BasePopover.Root>
      <BasePopover.Trigger render={trigger} />
      <BasePopover.Portal>
        <BasePopover.Positioner align={align} sideOffset={sideOffset} className="z-50">
          <BasePopover.Popup
            className={`min-w-[220px] rounded-[var(--radius-md)] border border-[var(--border-strong)] bg-[var(--surface)] p-2 text-[var(--fg)] shadow-[var(--shadow-lg)] transition-[opacity,transform] duration-150 data-[starting-style]:opacity-0 data-[starting-style]:-translate-y-1 data-[ending-style]:opacity-0 data-[ending-style]:-translate-y-1 ${className}`}
          >
            {children}
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}
```

---

## 6. 工作台布局与信息架构范式

```text
+-----------------------------------------------------------------------------+
| [Icon] AppName  [Sidebar]                          [Ctrl K] [中] [🎨] [⚙] [—□×] |  40px Titlebar
+--------------+--------------------------------------------------------------+
| Project ▾    | 视图标题 / ● 模块筛选 ×          [🔍 筛选] [列表|看板] [显示] [+ 新建] |  44px Topbar
+--------------+------------------------------------+-------------------------+
| · 跨项概览   | [未完成 12] [进行中] [待开始] [已完成]  [按状态|按目标|按优先级] |  主从分栏 (≥1024px)
| · 主工作台   |------------------------------------+-------------------------+
| · 监控/雷达  | + 输入标题，回车快速新建...          | ID · 类型    [主动作] [×] |
| · 大目标     |------------------------------------+ 标题（点击直接编辑）      |
| · 关系图     | ▾ 进行中 2                         | [状态一键切换条] [P0-P3]  |
|              | │ ◑ P1 ID-1 完整可读标题   [悬停动作] |                         |
| 模块         | ▾ 待开始 8                         | 核心说明 / 上下文 / 证据  |
| · 全部       | │ ○ P1 ID-2 完整可读标题   [悬停动作] | （内容置顶，零滚动即读）   |
| · 模块 A     |                                    |                         |
| · 模块 B     | ▾ 被阻塞 2                         | 关联依赖 / 上下游         |
|              | │ ⊘ P1 ID-3 完整可读标题  卡在 ID-1  | 低频表单属性（折叠/置底）  |
| [⚙] v0.1.0   | ↑↓ 切换 · C 新建 · / 搜索           | 底部粘性主操作栏           |
+--------------+------------------------------------+-------------------------+
```

**关键交互设计原则**：
1. **默认聚焦当下**：有历史/归档数据时，默认只展示「未完成 / 活跃」集合，把 70%–80% 已结束记录收进独立标签，底部留一个 1-click 入口 `查看 N 项已完成 →`。
2. **复用同一份 Detail 组件**：抽屉（`Drawer`）和分栏右侧（`Split`）只写一份 `DetailPanel`，宽屏零遮罩常驻，窄屏自动切滑出抽屉。
3. **弹窗轻量化**：新建弹窗默认只露 3–5 个最高频字段，打开自动聚焦标题、支持 `Ctrl+Enter` 提交，次级字段收进「更多选项」折叠区。
4. **告警不轰炸**：只把**需要人立刻拍板**的实时信号计入角标与置顶队列；低优先级历史差异降级折叠（如默认展示 Top 5 + `展开全部`）。

---

## 7. 给 AI 的 UI 验收自检清单（交付前逐条核对）

- [ ] **零硬编码颜色**：代码里没有散落的 `#hex`、`bg-white`、`text-gray-*`，浅色/深色/三套预设切换全部和谐。
- [ ] **窄窗口压力测试（`~960px` 宽）**：顶栏、侧边栏、列表行、卡片头部无任何 1 字 1 行竖排折行，无列内容向右溢出遮挡相邻列。
- [ ] **大数量压力测试（`60–200` 条数据）**：默认首屏不充满历史已完成噪音；长中文标题可读完整；不用反复开/关遮罩弹窗即可用 `↑ / ↓` 连续浏览。
- [ ] **高频操作 ≤ 1 步**：常用状态切换、主动作复制/执行、快速新建都能在当前视图直接完成，不必跳转 3 层菜单。
- [ ] **详情阅读顺序正确**：打开详情第一眼看到标题、一键状态条与核心内容，而不是先滚过一屏表单输入框。
- [ ] **无障碍与键盘**：`Esc` 逐层关闭浮层、`Ctrl+K` 呼出命令面板、`/` 聚焦搜索、`input/textarea` 聚焦时不误触发单字母快捷键。
