import type { ButtonHTMLAttributes } from "react";

type ButtonVariant = "primary" | "outline" | "ghost" | "accent";
type ButtonSize = "sm" | "md" | "icon";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    "bg-[var(--primary)] text-[var(--primary-fg)] hover:bg-[var(--primary-hover)] font-medium",
  outline:
    "bg-[var(--surface)] border border-[var(--border)] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] font-medium",
  ghost:
    "bg-transparent text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] font-medium",
  accent: "bg-[var(--accent)] text-white hover:brightness-105 font-medium",
};

const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: "h-6 px-2 text-[11.5px] gap-1 rounded-[var(--radius-xs)]",
  md: "h-7 px-2.5 text-[12px] gap-1.5 rounded-[var(--radius-sm)]",
  icon: "h-7 w-7 p-0 justify-center rounded-[var(--radius-sm)]",
};

export function Button({
  variant = "ghost",
  size = "md",
  className = "",
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex shrink-0 items-center whitespace-nowrap transition-colors duration-100 cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed ${VARIANT_CLASSES[variant]} ${SIZE_CLASSES[size]} ${className}`}
      {...props}
    />
  );
}
