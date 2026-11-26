import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { useT } from "@/i18n";
import { formatIpcError } from "@/lib/ipc";
import { resolveModule } from "@/lib/modules";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { ModuleAliasConfig } from "@/types/ipc";

const DEFAULT_MOD_SUGGESTIONS = [
  "engine",
  "desktop",
  "ui",
  "agent",
  "kg",
  "mcp",
  "release",
  "docs",
];

export function CreateIssueModal({
  modulePrefix,
  moduleAliases,
}: {
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
}) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const { createModalOpen, createParentId, activeModule, closeCreateModal, openDrawer, pushToast } =
    useUiStore();
  const { snapshot, byId, createIssue } = useWorkspaceStore();

  const [title, setTitle] = useState("");
  const [issueType, setIssueType] = useState("task");
  const [priority, setPriority] = useState(1);
  const [status, setStatus] = useState("open");
  const [assignee, setAssignee] = useState("");
  const [parentId, setParentId] = useState("");
  const [blockerId, setBlockerId] = useState("");
  const [selectedMods, setSelectedMods] = useState<string[]>([]);
  const [extraLabels, setExtraLabels] = useState("");
  const [description, setDescription] = useState("");
  const [acceptance, setAcceptance] = useState("");
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const prefix = modulePrefix || "mod:";

  useEffect(() => {
    if (!createModalOpen) return;
    setTitle("");
    setIssueType("task");
    setPriority(1);
    setStatus("open");
    setAssignee("");
    setBlockerId("");
    setExtraLabels("");
    setDescription("");
    setAcceptance("");
    setNotes("");

    const pid = createParentId ?? "";
    setParentId(pid);
    if (pid && byId[pid]?.modules.length) {
      setSelectedMods([...(byId[pid]?.modules ?? [])]);
    } else if (activeModule) {
      setSelectedMods([activeModule]);
    } else {
      setSelectedMods([]);
    }
  }, [createModalOpen, createParentId, activeModule, byId]);

  const modChoices = useMemo(() => {
    const set = new Set<string>();
    for (const m of snapshot?.modules ?? []) set.add(m.key);
    for (const m of selectedMods) set.add(m);
    if (set.size < 4) {
      for (const s of DEFAULT_MOD_SUGGESTIONS.slice(0, 5)) set.add(`${prefix}${s}`);
    }
    return [...set];
  }, [snapshot?.modules, selectedMods, prefix]);

  const handleParentChange = (nextPid: string) => {
    setParentId(nextPid);
    const parent = nextPid ? byId[nextPid] : undefined;
    if (parent && parent.modules.length > 0) {
      setSelectedMods([...parent.modules]);
    }
  };

  const toggleMod = (k: string) => {
    setSelectedMods((prev) => (prev.includes(k) ? prev.filter((x) => x !== k) : [...prev, k]));
  };

  const handleSubmit = async () => {
    const cleanTitle = title.trim();
    if (!cleanTitle) return;
    setSubmitting(true);
    try {
      const others = extraLabels
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      const id = await createIssue({
        title: cleanTitle,
        issueType,
        priority,
        status,
        assignee: assignee.trim() || undefined,
        parentId: parentId || undefined,
        labels: [...new Set([...selectedMods, ...others])],
        description: description.trim() || undefined,
        acceptanceCriteria: acceptance.trim() || undefined,
        notes: notes.trim() || undefined,
        blockedBy: blockerId ? [blockerId] : [],
      });
      closeCreateModal();
      pushToast(`${t.create.createdToast}: ${id ?? cleanTitle}`, "ok");
      if (id) openDrawer(id);
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setSubmitting(false);
    }
  };

  const openIssues = (snapshot?.issues ?? []).filter((x) => x.status !== "closed");
  const fieldCls =
    "w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 py-1.5 text-[12.5px] text-[var(--fg)] focus:border-[var(--border-strong)] focus:bg-[var(--surface)]";

  return (
    <Dialog
      open={createModalOpen}
      onOpenChange={(o) => {
        if (!o) closeCreateModal();
      }}
      title={t.create.title}
      maxWidthClass="max-w-[580px]"
      footer={
        <>
          <Button variant="outline" onClick={closeCreateModal}>
            {t.actions.cancel}
          </Button>
          <Button
            variant="primary"
            disabled={!title.trim() || submitting}
            onClick={() => void handleSubmit()}
          >
            {t.create.submit}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="space-y-1">
          <label htmlFor="create-title" className="text-[11.5px] font-medium text-[var(--fg-2)]">
            {t.create.fldTitle}
          </label>
          <input
            id="create-title"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t.create.titlePlaceholder}
            className={fieldCls}
          />
        </div>

        <div className="grid grid-cols-2 gap-2.5">
          <div className="space-y-1">
            <label htmlFor="create-type" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldType}
            </label>
            <select
              id="create-type"
              value={issueType}
              onChange={(e) => setIssueType(e.target.value)}
              className={fieldCls}
            >
              <option value="task">{t.types.task}</option>
              <option value="bug">{t.types.bug}</option>
              <option value="epic">{t.types.epic}</option>
              <option value="feature">{t.types.feature}</option>
              <option value="chore">{t.types.chore}</option>
            </select>
          </div>
          <div className="space-y-1">
            <label htmlFor="create-prio" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldPriority}
            </label>
            <select
              id="create-prio"
              value={priority}
              onChange={(e) => setPriority(Number(e.target.value))}
              className={fieldCls}
            >
              <option value={0}>P0</option>
              <option value={1}>P1</option>
              <option value={2}>P2</option>
              <option value={3}>P3</option>
            </select>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2.5">
          <div className="space-y-1">
            <label htmlFor="create-status" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldStatus}
            </label>
            <select
              id="create-status"
              value={status}
              onChange={(e) => setStatus(e.target.value)}
              className={fieldCls}
            >
              <option value="open">{t.statuses.open}</option>
              <option value="in_progress">{t.statuses.in_progress}</option>
              <option value="blocked">{t.statuses.blocked}</option>
            </select>
          </div>
          <div className="space-y-1">
            <label
              htmlFor="create-assignee"
              className="text-[11.5px] font-medium text-[var(--fg-2)]"
            >
              {t.create.fldAssignee}
            </label>
            <input
              id="create-assignee"
              type="text"
              value={assignee}
              onChange={(e) => setAssignee(e.target.value)}
              placeholder="claude / gpt / gemini"
              className={fieldCls}
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2.5">
          <div className="space-y-1">
            <label htmlFor="create-parent" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldParent}
            </label>
            <select
              id="create-parent"
              value={parentId}
              onChange={(e) => handleParentChange(e.target.value)}
              className={fieldCls}
            >
              <option value="">{t.create.noParent}</option>
              {(snapshot?.epics ?? []).map((ep) => (
                <option key={ep.id} value={ep.id}>
                  {ep.shortId} · {ep.title.slice(0, 32)}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-1">
            <label
              htmlFor="create-blocker"
              className="text-[11.5px] font-medium text-[var(--fg-2)]"
            >
              {t.create.fldBlocker}
            </label>
            <select
              id="create-blocker"
              value={blockerId}
              onChange={(e) => setBlockerId(e.target.value)}
              className={fieldCls}
            >
              <option value="">{t.create.noBlocker}</option>
              {openIssues.map((x) => (
                <option key={x.id} value={x.id}>
                  [{t.lanes[x.lane]}] {x.shortId} · {x.title.slice(0, 32)}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="space-y-1.5">
          <span className="text-[11.5px] font-medium text-[var(--fg-2)]">
            {t.create.fldModules}
          </span>
          <div className="flex flex-wrap gap-1.5">
            {modChoices.map((k) => {
              const m = resolveModule(k, prefix, locale, moduleAliases);
              const active = selectedMods.includes(k);
              return (
                <button
                  key={k}
                  type="button"
                  onClick={() => toggleMod(k)}
                  className={`inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 text-[11.5px] cursor-pointer ${
                    active
                      ? "border-[var(--accent-border)] bg-[var(--accent-soft)] font-medium text-[var(--accent)]"
                      : "border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-2)] hover:border-[var(--border-strong)]"
                  }`}
                >
                  <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: m.color }} />
                  <span>{m.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        <div className="space-y-1">
          <label htmlFor="create-labels" className="text-[11.5px] font-medium text-[var(--fg-2)]">
            {t.create.fldLabels}
          </label>
          <input
            id="create-labels"
            type="text"
            value={extraLabels}
            onChange={(e) => setExtraLabels(e.target.value)}
            placeholder="milestone-1, review"
            className={fieldCls}
          />
        </div>

        <div className="space-y-1">
          <label htmlFor="create-desc" className="text-[11.5px] font-medium text-[var(--fg-2)]">
            {t.create.fldDesc}
          </label>
          <textarea
            id="create-desc"
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className={fieldCls}
          />
        </div>

        <div className="grid grid-cols-2 gap-2.5">
          <div className="space-y-1">
            <label htmlFor="create-acc" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldAcc}
            </label>
            <textarea
              id="create-acc"
              rows={2}
              value={acceptance}
              onChange={(e) => setAcceptance(e.target.value)}
              className={fieldCls}
            />
          </div>
          <div className="space-y-1">
            <label htmlFor="create-notes" className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.create.fldNotes}
            </label>
            <textarea
              id="create-notes"
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className={fieldCls}
            />
          </div>
        </div>
      </div>
    </Dialog>
  );
}
