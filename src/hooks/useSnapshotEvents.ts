import { useEffect } from "react";
import { subscribeSnapshotUpdated } from "@/lib/ipc";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

export function useSnapshotEvents() {
  const applySnapshot = useWorkspaceStore((s) => s.applySnapshot);
  const refreshAllActive = useWorkspaceStore((s) => s.refreshAllActive);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void subscribeSnapshotUpdated((snap) => {
      if (!active) return;
      applySnapshot(snap);
      void refreshAllActive();
    }).then((fn) => {
      if (!active) fn();
      else unlisten = fn;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [applySnapshot, refreshAllActive]);
}
