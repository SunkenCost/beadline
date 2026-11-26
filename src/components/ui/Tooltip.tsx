import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";
import type { ReactNode } from "react";

export interface TooltipProps {
  label: string;
  children: ReactNode;
}

export function Tooltip({ label, children }: TooltipProps) {
  return (
    <BaseTooltip.Root>
      <BaseTooltip.Trigger render={<span className="inline-flex" />}>
        {children}
      </BaseTooltip.Trigger>
      <BaseTooltip.Portal>
        <BaseTooltip.Positioner sideOffset={6} className="z-50">
          <BaseTooltip.Popup className="rounded-[var(--radius-xs)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 py-1 text-[11px] text-[var(--fg)] shadow-[var(--shadow-sm)] transition-opacity duration-100 data-[starting-style]:opacity-0 data-[ending-style]:opacity-0">
            {label}
          </BaseTooltip.Popup>
        </BaseTooltip.Positioner>
      </BaseTooltip.Portal>
    </BaseTooltip.Root>
  );
}

export const TooltipProvider = BaseTooltip.Provider;
