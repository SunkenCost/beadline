import { useEffect } from "react";
import {
  subscribeAllActiveUpdated,
  subscribeRadarUpdated,
  subscribeSnapshotUpdated,
  subscribeWindowFocused,
} from "@/lib/ipc";
import { useRadarStore } from "@/stores/useRadarStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

const FOCUS_RESCAN_MIN_INTERVAL_MS = 10_000;

export function useSnapshotEvents() {
  const applySnapshot = useWorkspaceStore((s) => s.applySnapshot);
  const applyAllActive = useWorkspaceStore((s) => s.applyAllActive);
  const refreshAllActive = useWorkspaceStore((s) => s.refreshAllActive);
  const refreshWorkspaces = useWorkspaceStore((s) => s.refreshWorkspaces);
  const applyRadar = useRadarStore((s) => s.applyRadar);
  const refreshRadar = useRadarStore((s) => s.refreshRadar);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    void subscribeSnapshotUpdated((snap) => {
      if (!active) return;
      if (applySnapshot(snap)) {
        void refreshAllActive();
        // Refresh cached radar projections without spawning Git subprocesses
        void refreshRadar(snap.repoPath, false);
      }
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    void subscribeRadarUpdated((radar) => {
      if (!active) return;
      const activePath = useWorkspaceStore.getState().activePath;
      if (!activePath || radar.repoPath.toLowerCase() === activePath.toLowerCase()) {
        applyRadar(radar);
      }
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    void subscribeAllActiveUpdated((items) => {
      if (!active) return;
      if (applyAllActive(items)) {
        void refreshWorkspaces();
      }
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    // Throttled Git worktree re-scan ONLY on window re-focus (zero idle background polling!)
    void subscribeWindowFocused(() => {
      if (!active) return;
      const activePath = useWorkspaceStore.getState().activePath;
      const activeView = useUiStore.getState().activeView;
      const { lastGitScanAt, scanning } = useRadarStore.getState();
      if (
        activePath &&
        !scanning &&
        (activeView === "mission" || activeView === "board") &&
        Date.now() - lastGitScanAt >= FOCUS_RESCAN_MIN_INTERVAL_MS
      ) {
        void refreshRadar(activePath, true);
      }
    }).then((fn) => {
      if (!active) fn();
      else unlisteners.push(fn);
    });

    return () => {
      active = false;
      for (const fn of unlisteners) fn();
    };
  }, [
    applySnapshot,
    applyAllActive,
    refreshAllActive,
    refreshWorkspaces,
    applyRadar,
    refreshRadar,
  ]);
}
