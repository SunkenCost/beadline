import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { useT } from "@/i18n";
import { formatIpcError } from "@/lib/ipc";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { IssueDto } from "@/types/ipc";

export function CloseIssueDialog({
  issue,
  onDone,
}: {
  issue: IssueDto | null;
  onDone: () => void;
}) {
  const t = useT();
  const closeIssue = useWorkspaceStore((s) => s.closeIssue);
  const pushToast = useUiStore((s) => s.pushToast);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (issue) setReason(t.board.closeReasonDefault);
  }, [issue, t.board.closeReasonDefault]);

  if (!issue) return null;

  const handleConfirm = async () => {
    setBusy(true);
    try {
      await closeIssue(issue.id, reason.trim() || undefined);
      pushToast(`${t.lanes.closed}: ${issue.shortId}`, "ok");
      onDone();
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={Boolean(issue)}
      onOpenChange={(o) => {
        if (!o) onDone();
      }}
      title={t.board.confirmCloseTitle}
      description={`${issue.shortId} · ${issue.title}`}
      maxWidthClass="max-w-[440px]"
      footer={
        <>
          <Button variant="outline" onClick={onDone}>
            {t.actions.cancel}
          </Button>
          <Button variant="accent" disabled={busy} onClick={() => void handleConfirm()}>
            {t.actions.markDone}
          </Button>
        </>
      }
    >
      <textarea
        rows={3}
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder={t.board.closeReasonPlaceholder}
        className="w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-2.5 text-[12.5px] text-[var(--fg)] focus:border-[var(--border-strong)]"
      />
    </Dialog>
  );
}
