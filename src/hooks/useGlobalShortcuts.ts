import { useEffect } from "react";
import { useUiStore } from "@/stores/useUiStore";

export function useGlobalShortcuts(focusSearch: () => void) {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ui = useUiStore.getState();

      // Always allow Cmd/Ctrl+K and Cmd/Ctrl+,
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        ui.setPaletteOpen(!ui.paletteOpen);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        ui.setSettingsOpen(true);
        return;
      }

      if (e.key === "Escape") {
        if (ui.paletteOpen) ui.setPaletteOpen(false);
        else if (ui.settingsOpen) ui.setSettingsOpen(false);
        else if (ui.createModalOpen) ui.closeCreateModal();
        else if (ui.selectedIssueId) ui.closeDrawer();
        return;
      }

      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase() ?? "";
      const typing =
        tag === "input" ||
        tag === "textarea" ||
        tag === "select" ||
        target?.isContentEditable === true;
      if (typing || e.ctrlKey || e.metaKey || e.altKey) return;

      if (e.key === "/") {
        e.preventDefault();
        focusSearch();
      } else if (e.key === "c" || e.key === "C") {
        e.preventDefault();
        ui.openCreateModal();
      } else if (e.key === "m" || e.key === "M") {
        ui.setActiveView("mission");
      } else if (e.key === "1") {
        ui.setActiveView("tasks");
      } else if (e.key === "2") {
        ui.setActiveView("epics");
      } else if (e.key === "3") {
        ui.setActiveView("dag");
      } else if (e.key === "b" || e.key === "B") {
        if (ui.activeView === "tasks") {
          ui.setTasksMode(ui.tasksMode === "list" ? "board" : "list");
        } else {
          ui.setActiveView("tasks");
          ui.setTasksMode("board");
        }
      } else if (e.key === "0") {
        ui.setActiveView("all-active");
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [focusSearch]);
}
