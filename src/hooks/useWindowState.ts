import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import { isTauriRuntime } from "@/lib/ipc";

export function useWindowState() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let active = true;
    const win = getCurrentWindow();

    void win
      .isMaximized()
      .then((v) => {
        if (active) setMaximized(v);
      })
      .catch(() => undefined);

    let unlisten: (() => void) | undefined;
    void win
      .onResized(() => {
        void win
          .isMaximized()
          .then((v) => {
            if (active) setMaximized(v);
          })
          .catch(() => undefined);
      })
      .then((fn) => {
        if (!active) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const minimize = useCallback(() => {
    if (!isTauriRuntime()) return;
    void getCurrentWindow().minimize();
  }, []);

  const toggleMaximize = useCallback(() => {
    if (!isTauriRuntime()) return;
    void getCurrentWindow().toggleMaximize();
  }, []);

  const close = useCallback(() => {
    if (!isTauriRuntime()) return;
    void getCurrentWindow().close();
  }, []);

  return { maximized, minimize, toggleMaximize, close };
}
