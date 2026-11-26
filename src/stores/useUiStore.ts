import { create } from "zustand";
import type { BoardLane } from "@/types/ipc";

export type ActiveView = "mission" | "all-active" | "board" | "epics" | "dag" | "list";
export type BoardScope = "tasks" | "all";
export type BoardGroupMode = "flat" | "epic";
export type DagMode = "connected" | "active" | "all";

export interface ToastItem {
  id: number;
  message: string;
  kind: "info" | "ok" | "err";
}

interface UiState {
  activeView: ActiveView;
  sidebarCollapsed: boolean;
  settingsOpen: boolean;
  paletteOpen: boolean;
  createModalOpen: boolean;
  createParentId: string | null;
  selectedIssueId: string | null;
  activeModule: string;
  boardScope: BoardScope;
  boardGroupMode: BoardGroupMode;
  hideClosedRounds: boolean;
  dagMode: DagMode;
  search: string;
  listLaneFilter: BoardLane | "";
  toasts: ToastItem[];

  setActiveView: (view: ActiveView) => void;
  toggleSidebar: () => void;
  setSettingsOpen: (open: boolean) => void;
  setPaletteOpen: (open: boolean) => void;
  openCreateModal: (parentId?: string | null) => void;
  closeCreateModal: () => void;
  openDrawer: (issueId: string) => void;
  closeDrawer: () => void;
  setActiveModule: (mod: string) => void;
  setBoardScope: (scope: BoardScope) => void;
  setBoardGroupMode: (mode: BoardGroupMode) => void;
  setHideClosedRounds: (hide: boolean) => void;
  setDagMode: (mode: DagMode) => void;
  setSearch: (q: string) => void;
  setListLaneFilter: (lane: BoardLane | "") => void;
  pushToast: (message: string, kind?: "info" | "ok" | "err") => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 1;

export const useUiStore = create<UiState>((set) => ({
  activeView: "board",
  sidebarCollapsed: false,
  settingsOpen: false,
  paletteOpen: false,
  createModalOpen: false,
  createParentId: null,
  selectedIssueId: null,
  activeModule: "",
  boardScope: "tasks",
  boardGroupMode: "flat",
  hideClosedRounds: false,
  dagMode: "connected",
  search: "",
  listLaneFilter: "",
  toasts: [],

  setActiveView: (activeView) => set({ activeView }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  openCreateModal: (parentId = null) =>
    set({ createModalOpen: true, createParentId: parentId ?? null }),
  closeCreateModal: () => set({ createModalOpen: false, createParentId: null }),
  openDrawer: (selectedIssueId) => set({ selectedIssueId }),
  closeDrawer: () => set({ selectedIssueId: null }),
  setActiveModule: (activeModule) => set({ activeModule }),
  setBoardScope: (boardScope) => set({ boardScope }),
  setBoardGroupMode: (boardGroupMode) => set({ boardGroupMode }),
  setHideClosedRounds: (hideClosedRounds) => set({ hideClosedRounds }),
  setDagMode: (dagMode) => set({ dagMode }),
  setSearch: (search) => set({ search }),
  setListLaneFilter: (listLaneFilter) => set({ listLaneFilter }),

  pushToast: (message, kind = "info") => {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { id, message, kind }] }));
    window.setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 2800);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
