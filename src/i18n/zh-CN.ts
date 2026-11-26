export const zhCN = {
  appName: "Beadline",
  nav: {
    allActive: "全部项目进行中",
    board: "任务看板",
    epics: "主目标进度",
    dag: "依赖图谱",
    list: "全部清单",
    modules: "大模块",
    activeNow: "执行中",
    allModules: "全部模块",
    settings: "设置",
  },
  lanes: {
    in_progress: "进行中",
    ready: "就绪可派发",
    blocked: "被阻塞",
    closed: "已完成",
  },
  actions: {
    newIssue: "新建",
    commandPalette: "命令面板",
    syncGithub: "同步 GitHub",
    collapseSidebar: "折叠或展开侧边栏",
    minimize: "最小化",
    maximize: "最大化",
    restore: "还原窗口",
    close: "关闭",
    switchLanguage: "切换语言",
    switchTheme: "外观与主题",
  },
  theme: {
    presets: {
      "warm-claude": "Warm Claude",
      "linear-slate": "Linear Slate",
      "pure-mono": "Pure Mono",
    },
    appearance: {
      light: "浅色",
      dark: "深色",
      system: "跟随系统",
    },
    radius: {
      sharp: "利落",
      default: "标准",
      soft: "柔和",
    },
    density: {
      compact: "紧凑",
      comfortable: "舒展",
    },
    sectionPreset: "主题预设",
    sectionAppearance: "明暗模式",
    sectionDensity: "排版密度",
  },
  status: {
    engineStandby: "内核就绪",
    milestoneHint: "骨架与无边框原生窗口已就绪 · 下一步接入 Rust Beads 混合引擎",
    desktopReadyTitle: "Tauri v2 无边框桌面骨架已就绪",
    desktopReadyDesc:
      "已内置 Coss UI (@base-ui/react) 原语、三套语义主题（Warm Claude / Linear Slate / Pure Mono）、中英双语与窗口几何记忆。下一步（bl-84n.2）将接入 Rust notify 事件监听引擎并实时读取本地 Beads 仓库。",
  },
} as const;

export type I18nDict = typeof zhCN;
