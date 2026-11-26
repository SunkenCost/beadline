import { Popover as BasePopover } from "@base-ui/react/popover";
import type { ReactNode } from "react";

export interface PopoverProps {
  trigger: ReactNode;
  children: ReactNode;
  align?: "start" | "center" | "end";
  sideOffset?: number;
  className?: string;
}

export function Popover({
  trigger,
  children,
  align = "end",
  sideOffset = 6,
  className = "",
}: PopoverProps) {
  return (
    <BasePopover.Root>
      <BasePopover.Trigger render={<span className="inline-flex" />}>{trigger}</BasePopover.Trigger>
      <BasePopover.Portal>
        <BasePopover.Positioner align={align} sideOffset={sideOffset} className="z-50">
          <BasePopover.Popup
            className={`min-w-[220px] rounded-[var(--radius-md)] border border-[var(--border-strong)] bg-[var(--surface)] p-2 text-[var(--fg)] shadow-[var(--shadow-lg)] transition-[opacity,transform] duration-150 data-[starting-style]:opacity-0 data-[starting-style]:-translate-y-1 data-[ending-style]:opacity-0 data-[ending-style]:-translate-y-1 ${className}`}
          >
            {children}
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}
