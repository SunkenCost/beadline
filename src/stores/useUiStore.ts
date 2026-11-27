import { create } from "zustand";
import type { BoardLane } from "@/types/ipc";

export type ActiveView = "tasks" | "mission" | "all-active" | "epics" | "dag";
export type TasksMode = "list" | "board";
export type TasksFilter = "open" | "in_progress" | "ready" | "blocked" | "closed" | "all";
export type TasksGroupBy = "status" | "epic" | "priority";
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
  tasksMode: TasksMode;
  tasksFilter: TasksFilter;
  tasksGroupBy: TasksGroupBy;
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
  closedLaneCollapsed: boolean;
  dagMode: DagMode;
  search: string;
  listLaneFilter: BoardLane | "";
  toasts: ToastItem[];

  setActiveView: (view: ActiveView) => void;
  setTasksMode: (mode: TasksMode) => void;
  setTasksFilter: (filter: TasksFilter) => void;
  setTasksGroupBy: (groupBy: TasksGroupBy) => void;
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
  setClosedLaneCollapsed: (collapsed: boolean) => void;
  toggleClosedLane: () => void;
  setDagMode: (mode: DagMode) => void;
  setSearch: (q: string) => void;
  setListLaneFilter: (lane: BoardLane | "") => void;
  pushToast: (message: string, kind?: "info" | "ok" | "err") => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 1;

export const useUiStore = create<UiState>((set) => ({
  activeView: "tasks",
  tasksMode: "list",
  tasksFilter: "open",
  tasksGroupBy: "status",
  sidebarCollapsed: false,
  settingsOpen: false,
  paletteOpen: false,
  createModalOpen: false,
  createParentId: null,
  selectedIssueId: null,
  activeModule: "",
  boardScope: "tasks",
  boardGroupMode: "flat",
  hideClosedRounds: true,
  closedLaneCollapsed: false,
  dagMode: "connected",
  search: "",
  listLaneFilter: "",
  toasts: [],

  setActiveView: (activeView) => set({ activeView }),
  setTasksMode: (tasksMode) => set({ tasksMode }),
  setTasksFilter: (tasksFilter) => set({ tasksFilter }),
  setTasksGroupBy: (tasksGroupBy) => set({ tasksGroupBy }),
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
  setClosedLaneCollapsed: (closedLaneCollapsed) => set({ closedLaneCollapsed }),
  toggleClosedLane: () => set((s) => ({ closedLaneCollapsed: !s.closedLaneCollapsed })),
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
