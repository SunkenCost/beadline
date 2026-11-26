import type { IssueDto } from "@/types/ipc";

/**
 * Extracts a concise, plain-language one-liner from raw Markdown text so humans
 * can immediately grasp what an issue or parent epic is about without reading
 * dense technical sections.
 */
export function extractPlainSummary(markdown: string, maxChars = 120): string {
  if (!markdown.trim()) return "";

  const lines = markdown
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => {
      if (!l) return false;
      if (l.startsWith("```")) return false;
      if (/^#{1,6}\s+/.test(l)) return false;
      if (/^[-*_]{3,}$/.test(l)) return false;
      return true;
    });

  const first = lines[0];
  if (!first) return "";

  const cleaned = first
    .replace(/^[-*+]\s+\[[ xX]\]\s+/, "")
    .replace(/^[-*+]\s+/, "")
    .replace(/^\d+\.\s+/, "")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\s+/g, " ")
    .trim();

  if (cleaned.length <= maxChars) return cleaned;
  return `${cleaned.slice(0, maxChars - 1)}…`;
}

/**
 * Turns a verbose epic title (e.g. "[Release v3.7.1-p5] 九册三合一权威发布图包与最终主干合流收口")
 * into a short, human-scannable label for Kanban cards.
 */
export function shortParentLabel(parentTitle: string, maxChars = 18): string {
  const stripped = parentTitle
    .replace(/^\[[^\]]+\]\s*/, "")
    .replace(/^Milestone\s+\d+\s*[:：]\s*/i, "")
    .trim();
  const base = stripped || parentTitle.trim();
  if (base.length <= maxChars) return base;
  return `${base.slice(0, maxChars - 1)}…`;
}

const ROUND_NOISE_RE =
  /(?:Wave\s*\d+\s*(?:r\d+|终审|微修|复验)|\(r\d+(?:→r\d+)?\)|\br[2-9]\b.*(?:终审|微修|复验|审))/i;

/**
 * Identifies closed, fine-grained AI agent iteration-round cards (e.g. Wave 12 r2/r3/r4/r5)
 * so users can optionally fold iteration noise on the Kanban board while keeping real goals visible.
 */
export function isClosedIterationRound(issue: IssueDto): boolean {
  if (issue.lane !== "closed" || issue.issueType === "epic") return false;
  return ROUND_NOISE_RE.test(issue.title);
}
