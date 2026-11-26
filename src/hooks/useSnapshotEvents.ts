import { useEffect } from "react";
import { subscribeAllActiveUpdated, subscribeSnapshotUpdated } from "@/lib/ipc";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

export function useSnapshotEvents() {
  const applySnapshot = useWorkspaceStore((s) => s.applySnapshot);
  const applyAllActive = useWorkspaceStore((s) => s.applyAllActive);
  const refreshAllActive = useWorkspaceStore((s) => s.refreshAllActive);
  const refreshWorkspaces = useWorkspaceStore((s) => s.refreshWorkspaces);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    void subscribeSnapshotUpdated((snap) => {
      if (!active) return;
      applySnapshot(snap);
      void refreshAllActive();
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    void subscribeAllActiveUpdated((items) => {
      if (!active) return;
      applyAllActive(items);
      void refreshWorkspaces();
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    return () => {
      active = false;
      for (const fn of unlisteners) fn();
    };
  }, [applySnapshot, applyAllActive, refreshAllActive, refreshWorkspaces]);
}
