import { create } from "zustand";

export type ActiveView = "all-active" | "board" | "epics" | "dag" | "list";

interface UiState {
  activeView: ActiveView;
  sidebarCollapsed: boolean;
  setActiveView: (view: ActiveView) => void;
  toggleSidebar: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeView: "board",
  sidebarCollapsed: false,
  setActiveView: (activeView) => set({ activeView }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
