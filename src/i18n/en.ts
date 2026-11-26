export const en = {
  appName: "Beadline",
  nav: {
    allActive: "All Projects · Active",
    board: "Board",
    epics: "Epics",
    dag: "Dependency Graph",
    list: "All Issues",
    modules: "Modules",
    activeNow: "In Progress",
    allModules: "All Modules",
    settings: "Settings",
  },
  lanes: {
    in_progress: "In Progress",
    ready: "Ready",
    blocked: "Blocked",
    closed: "Closed",
  },
  actions: {
    newIssue: "New",
    commandPalette: "Command Palette",
    syncGithub: "Sync GitHub",
    collapseSidebar: "Toggle Sidebar",
    minimize: "Minimize",
    maximize: "Maximize",
    restore: "Restore",
    close: "Close",
    switchLanguage: "Switch Language",
    switchTheme: "Theme & Appearance",
  },
  theme: {
    presets: {
      "warm-claude": "Warm Claude",
      "linear-slate": "Linear Slate",
      "pure-mono": "Pure Mono",
    },
    appearance: {
      light: "Light",
      dark: "Dark",
      system: "System",
    },
    radius: {
      sharp: "Sharp",
      default: "Default",
      soft: "Soft",
    },
    density: {
      compact: "Compact",
      comfortable: "Comfortable",
    },
    sectionPreset: "Theme Preset",
    sectionAppearance: "Appearance",
    sectionDensity: "Density",
  },
  status: {
    engineStandby: "Shell Ready",
    milestoneHint: "Frameless desktop shell ready · Next up: Rust Beads hybrid engine",
    desktopReadyTitle: "Tauri v2 Frameless Shell Ready",
    desktopReadyDesc:
      "Coss UI (@base-ui/react) primitives, 3 semantic theme presets (Warm Claude / Linear Slate / Pure Mono), bilingual i18n, and persistent window geometry are live. Next task (bl-84n.2) connects the Rust notify hybrid Beads engine.",
  },
} as const;
