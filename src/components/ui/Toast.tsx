import { useUiStore } from "@/stores/useUiStore";

export function ToastStack() {
  const toasts = useUiStore((s) => s.toasts);
  const dismiss = useUiStore((s) => s.dismissToast);

  if (toasts.length === 0) return null;

  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-[80] flex flex-col gap-1.5">
      {toasts.map((t) => (
        <button
          key={t.id}
          type="button"
          onClick={() => dismiss(t.id)}
          className={`pointer-events-auto max-w-[400px] rounded-[var(--radius-sm)] bg-[var(--primary)] px-3 py-2 text-left text-[12px] font-medium text-[var(--primary-fg)] shadow-[var(--shadow-lg)] cursor-pointer ${
            t.kind === "ok"
              ? "border-l-[3px] border-l-[var(--success)]"
              : t.kind === "err"
                ? "border-l-[3px] border-l-[var(--danger)]"
                : ""
          }`}
        >
          {t.message}
        </button>
      ))}
    </div>
  );
}
