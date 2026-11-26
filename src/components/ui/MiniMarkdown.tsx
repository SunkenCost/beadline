import { CheckSquare, Square } from "lucide-react";
import { Fragment, useMemo } from "react";
import { formatIpcError, ipcOpenExternalLink } from "@/lib/ipc";
import { type InlineNode, parseMarkdownBlocks } from "@/lib/markdown";
import { useUiStore } from "@/stores/useUiStore";

function RenderInlines({ nodes }: { nodes: InlineNode[] }) {
  const pushToast = useUiStore((s) => s.pushToast);

  return (
    <>
      {nodes.map((node, idx) => {
        const key = `${node.type}-${idx}`;
        if (node.type === "bold") {
          return (
            <strong key={key} className="font-semibold text-[var(--fg)]">
              {node.text}
            </strong>
          );
        }
        if (node.type === "code") {
          return (
            <code
              key={key}
              className="rounded-[3px] border border-[var(--border)] bg-[var(--surface)] px-1 py-0.5 font-mono-tabular text-[11px] text-[var(--accent)]"
            >
              {node.text}
            </code>
          );
        }
        if (node.type === "link") {
          return (
            <button
              key={key}
              type="button"
              onClick={() =>
                void ipcOpenExternalLink(node.url).catch((e) => pushToast(formatIpcError(e), "err"))
              }
              title={node.url}
              className="inline text-left text-[var(--accent)] underline underline-offset-2 hover:brightness-110 cursor-pointer"
            >
              {node.label}
            </button>
          );
        }
        return <Fragment key={key}>{node.text}</Fragment>;
      })}
    </>
  );
}

export function MiniMarkdown({
  content,
  emptyText,
  tone = "default",
}: {
  content: string;
  emptyText?: string;
  tone?: "default" | "muted" | "success";
}) {
  const blocks = useMemo(() => parseMarkdownBlocks(content), [content]);

  const containerCls =
    tone === "success"
      ? "rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--success-soft)] p-3 text-[12.5px] leading-relaxed text-[var(--fg)] select-text"
      : tone === "muted"
        ? "rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3 text-[12px] leading-relaxed text-[var(--fg-2)] select-text"
        : "rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3 text-[12.5px] leading-relaxed text-[var(--fg)] select-text";

  if (!content.trim() || blocks.length === 0) {
    return (
      <div className={containerCls}>
        <span className="text-[var(--fg-3)]">{emptyText ?? ""}</span>
      </div>
    );
  }

  return (
    <div className={`${containerCls} space-y-2 break-words`}>
      {blocks.map((block, idx) => {
        const key = `${block.type}-${idx}`;
        if (block.type === "heading") {
          return (
            <div
              key={key}
              className={
                block.level === 2
                  ? "pt-1 text-[13.5px] font-semibold text-[var(--fg)]"
                  : "pt-0.5 text-[12.5px] font-semibold text-[var(--fg)]"
              }
            >
              <RenderInlines nodes={block.inlines} />
            </div>
          );
        }
        if (block.type === "checklist") {
          return (
            <div key={key} className="flex items-start gap-2">
              {block.checked ? (
                <CheckSquare className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--success)]" />
              ) : (
                <Square className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
              )}
              <span className={block.checked ? "text-[var(--fg-3)] line-through" : ""}>
                <RenderInlines nodes={block.inlines} />
              </span>
            </div>
          );
        }
        if (block.type === "bullet") {
          return (
            <div key={key} className="flex items-start gap-2 pl-0.5">
              <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-[var(--fg-3)]" />
              <span className="min-w-0 flex-1">
                <RenderInlines nodes={block.inlines} />
              </span>
            </div>
          );
        }
        if (block.type === "codeblock") {
          return (
            <pre
              key={key}
              className="overflow-x-auto rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2.5 py-2 font-mono-tabular text-[11px] leading-relaxed text-[var(--fg)]"
            >
              <code>{block.code}</code>
            </pre>
          );
        }
        return (
          <p key={key} className="whitespace-pre-wrap">
            <RenderInlines nodes={block.inlines} />
          </p>
        );
      })}
    </div>
  );
}
