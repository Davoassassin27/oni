import {
  useEffect,
  useState,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { cn, stateLabel, stateTone } from "../lib/utils";
import type { TaskState } from "../lib/types";
import { XIcon } from "./icons";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "icon";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  className,
  children,
  disabled,
  ...props
}: ButtonProps) {
  const variants: Record<ButtonVariant, string> = {
    primary:
      "border border-kintsugi bg-kintsugi text-sumi hover:bg-[#d4ad3a] active:bg-[#b8931f]",
    secondary:
      "border border-hueso/20 bg-transparent text-hueso hover:border-hueso/35 hover:bg-hueso/5",
    ghost:
      "border border-transparent bg-transparent text-ceniza hover:bg-hueso/5 hover:text-hueso",
    danger:
      "border border-vermilion/40 bg-transparent text-vermilion hover:bg-vermilion/10",
  };
  const sizes: Record<ButtonSize, string> = {
    sm: "h-9 px-3 text-xs",
    md: "h-11 px-4 text-sm",
    icon: "h-11 w-11 p-0",
  };

  return (
    <button
      type="button"
      className={cn(
        "inline-flex select-none items-center justify-center gap-2 rounded-lg font-medium transition-colors disabled:pointer-events-none disabled:opacity-45",
        variants[variant],
        sizes[size],
        className,
      )}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Spinner /> : null}
      {children}
    </button>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className={cn("h-4 w-4 animate-spin motion-reduce:animate-none", className)}
      aria-hidden="true"
    >
      <circle
        cx="12"
        cy="12"
        r="8"
        stroke="currentColor"
        strokeWidth="2"
        className="opacity-25"
      />
      <path
        d="M20 12a8 8 0 0 0-8-8"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  );
}

const pillTones = {
  active: "border-kintsugi/30 bg-kintsugi/10 text-kintsugi",
  review: "border-hueso/25 bg-hueso/10 text-hueso",
  done: "border-hueso/15 bg-transparent text-ceniza",
  danger: "border-vermilion/30 bg-vermilion/10 text-vermilion",
} as const;

const dotTones = {
  active: "bg-kintsugi animate-pulse motion-reduce:animate-none",
  review: "bg-hueso",
  done: "bg-ceniza",
  danger: "bg-vermilion",
} as const;

export function StatePill({
  state,
  className,
}: {
  state: TaskState;
  className?: string;
}) {
  const tone = stateTone(state);
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 font-mono text-[10px] font-medium uppercase tracking-[0.14em]",
        pillTones[tone],
        className,
      )}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", dotTones[tone])} />
      {stateLabel(state)}
    </span>
  );
}

export const inputClass =
  "h-11 w-full rounded-lg border border-hueso/15 bg-sunken px-3 text-[16px] text-hueso placeholder:text-ceniza/50 transition-colors focus:border-kintsugi/60 focus:outline-none focus:ring-1 focus:ring-kintsugi/40 md:text-sm";

export const textareaClass =
  "min-h-28 w-full rounded-lg border border-hueso/15 bg-sunken px-3 py-2.5 text-[16px] leading-relaxed text-hueso placeholder:text-ceniza/50 transition-colors focus:border-kintsugi/60 focus:outline-none focus:ring-1 focus:ring-kintsugi/40 md:text-sm";

export const selectClass =
  "h-11 w-full appearance-none rounded-lg border border-hueso/15 bg-sunken px-3 pr-9 text-[16px] text-hueso transition-colors focus:border-kintsugi/60 focus:outline-none focus:ring-1 focus:ring-kintsugi/40 md:text-sm";

export function Field({
  label,
  htmlFor,
  hint,
  error,
  children,
  className,
}: {
  label: string;
  htmlFor?: string;
  hint?: ReactNode;
  error?: string | null;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("space-y-2", className)}>
      <label
        htmlFor={htmlFor}
        className="block font-mono text-[10px] font-medium uppercase tracking-[0.16em] text-ceniza"
      >
        {label}
      </label>
      {children}
      {error ? (
        <p className="text-xs text-vermilion">{error}</p>
      ) : hint ? (
        <p className="text-xs leading-relaxed text-ceniza/75">{hint}</p>
      ) : null}
    </div>
  );
}

export function Sheet({
  open,
  onClose,
  title,
  description,
  footer,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.body.style.overflow = previous;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center md:items-center md:p-6">
      <button
        type="button"
        aria-label="Close"
        className="absolute inset-0 animate-fade bg-sumi/80"
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="relative z-10 flex max-h-[92dvh] w-full animate-sheet flex-col overflow-hidden rounded-t-2xl border border-hueso/15 bg-raised md:max-w-lg md:rounded-2xl"
      >
        <header className="flex items-start justify-between gap-4 border-b border-hueso/10 px-5 py-4">
          <div className="space-y-1">
            <h2 className="text-lg font-semibold tracking-tight text-hueso">
              {title}
            </h2>
            {description ? (
              <p className="text-pretty text-xs leading-relaxed text-ceniza">
                {description}
              </p>
            ) : null}
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="-mr-2 -mt-2 shrink-0"
            onClick={onClose}
            aria-label="Close"
          >
            <XIcon className="h-5 w-5" />
          </Button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-5">
          {children}
        </div>
        {footer ? (
          <div className="shrink-0 border-t border-hueso/10 px-5 pb-[max(env(safe-area-inset-bottom),1rem)] pt-4">
            {footer}
          </div>
        ) : null}
      </div>
    </div>
  );
}

type ToastTone = "default" | "success" | "error";
interface ToastItem {
  id: number;
  message: string;
  tone: ToastTone;
}

export function toast(message: string, tone: ToastTone = "default"): void {
  window.dispatchEvent(
    new CustomEvent("oni:toast", { detail: { message, tone } }),
  );
}

export function Toaster() {
  const [items, setItems] = useState<ToastItem[]>([]);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ message: string; tone: ToastTone }>)
        .detail;
      const id = Date.now() + Math.random();
      setItems((current) => [...current, { id, ...detail }]);
      window.setTimeout(() => {
        setItems((current) => current.filter((item) => item.id !== id));
      }, 4200);
    };
    window.addEventListener("oni:toast", handler);
    return () => window.removeEventListener("oni:toast", handler);
  }, []);

  return (
    <div
      aria-live="polite"
      className="pointer-events-none fixed inset-x-0 bottom-24 z-[60] flex flex-col items-center gap-2 px-4 md:bottom-6"
    >
      {items.map((item) => (
        <div
          key={item.id}
          className={cn(
            "animate-rise rounded-lg border px-4 py-2.5 text-sm",
            item.tone === "error"
              ? "border-vermilion/35 bg-raised text-vermilion"
              : item.tone === "success"
                ? "border-kintsugi/35 bg-raised text-kintsugi"
                : "border-hueso/20 bg-raised text-hueso",
          )}
        >
          {item.message}
        </div>
      ))}
    </div>
  );
}

export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  description,
  confirmLabel,
  tone = "primary",
  loading = false,
  children,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description?: ReactNode;
  confirmLabel: string;
  tone?: "primary" | "danger";
  loading?: boolean;
  children?: ReactNode;
}) {
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={title}
      description={description}
    >
      {children}
      <div className={cn("flex gap-2", children ? "mt-6" : "mt-2")}>
        <Button variant="secondary" className="flex-1" onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant={tone === "danger" ? "danger" : "primary"}
          className="flex-1"
          onClick={onConfirm}
          loading={loading}
        >
          {confirmLabel}
        </Button>
      </div>
    </Sheet>
  );
}

export function ConnectionBadge({
  online,
}: {
  online: boolean | null;
}) {
  const label = online === null ? "connecting" : online ? "daemon" : "offline";
  return (
    <span
      className="inline-flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.16em] text-ceniza"
      aria-live="polite"
    >
      <span
        className={cn(
          "h-1.5 w-1.5 rounded-full",
          online === null
            ? "bg-ceniza"
            : online
              ? "animate-pulse bg-kintsugi motion-reduce:animate-none"
              : "bg-vermilion",
        )}
      />
      {label}
    </span>
  );
}
