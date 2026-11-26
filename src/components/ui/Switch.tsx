import { Switch as BaseSwitch } from "@base-ui/react/switch";

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  ariaLabel: string;
}

export function Switch({ checked, onCheckedChange, ariaLabel }: SwitchProps) {
  return (
    <BaseSwitch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      aria-label={ariaLabel}
      className="relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-[var(--border-strong)] bg-[var(--surface-subtle)] p-0.5 transition-colors duration-150 data-[checked]:border-[var(--accent)] data-[checked]:bg-[var(--accent)] cursor-pointer"
    >
      <BaseSwitch.Thumb className="block h-3.5 w-3.5 rounded-full bg-[var(--fg-2)] transition-transform duration-150 data-[checked]:translate-x-4 data-[checked]:bg-white" />
    </BaseSwitch.Root>
  );
}
