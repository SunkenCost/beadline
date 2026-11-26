import { create } from "zustand";

export type ActiveView = "all-active" | "board" | "epics" | "dag" | "list";

interface UiState {
  activeView: ActiveView;
  sidebarCollapsed: boolean;
  settingsOpen: boolean;
  setActiveView: (view: ActiveView) => void;
  toggleSidebar: () => void;
  setSettingsOpen: (open: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeView: "board",
  sidebarCollapsed: false,
  settingsOpen: false,
  setActiveView: (activeView) => set({ activeView }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
}));
