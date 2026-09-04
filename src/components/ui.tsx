import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";


const LABEL_LINE = 11.5 * 1.375;
const TOGGLE_LINE = 12 * 1.375;

export function CheckIcon({ size = 10 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path
        d="M2.5 6.2 L4.8 8.5 L9.5 3.5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function ChevronIcon({ size = 10, open }: { size?: number; open?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 12 12"
      fill="none"
      aria-hidden="true"
      style={{ transform: open ? "rotate(180deg)" : undefined, transition: "transform 140ms ease" }}
    >
      <path
        d="M3 4.5 L6 7.8 L9 4.5"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

interface CheckboxProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: React.ReactNode;
  hint?: React.ReactNode;
  disabled?: boolean;
  stopPropagation?: boolean;
  ariaLabel?: string;
}

export function Checkbox({
  checked, onChange, label, hint, disabled, stopPropagation, ariaLabel,
}: CheckboxProps) {
  const box = (
    <span
      className="zr-check"
      data-checked={checked}
      aria-hidden="true"
      style={{ opacity: disabled ? 0.45 : 1 }}
    >
      {checked && <CheckIcon />}
    </span>
  );

  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)}
      disabled={disabled}
      className={`flex gap-1.5 text-left ${hint ? "items-start" : "items-center"}`}
      style={{ color: "var(--text-2)", cursor: disabled ? "not-allowed" : "pointer" }}
      onClick={(e) => {
        if (stopPropagation) e.stopPropagation();
        if (!disabled) onChange(!checked);
      }}
    >


      <span
        className="shrink-0 flex items-center"
        style={{ height: hint ? LABEL_LINE : undefined }}
      >
        {box}
      </span>
      {(label || hint) && (
        <span className="min-w-0">
          {label && <span className="text-[11.5px] leading-snug">{label}</span>}
          {hint && (
            <span className="block text-[11px] leading-snug" style={{ color: "var(--text-3)" }}>
              {hint}
            </span>
          )}
        </span>
      )}
    </button>
  );
}

export interface Option<T extends string = string> {
  value: T;
  label: string;
  hint?: string;
}

interface SelectProps<T extends string> {
  value: T;
  onChange: (v: T) => void;
  options: Option<T>[];
  width?: number | string;
  placeholder?: string;
  ariaLabel?: string;
  align?: "left" | "right";
}

export function Select<T extends string>({
  value, onChange, options, width, placeholder, ariaLabel, align = "left",
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const btnRef = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setOpen(false), []);

  const current = options.find((o) => o.value === value);

  useEffect(() => {
    if (open) setActive(Math.max(0, options.findIndex((o) => o.value === value)));

  }, [open]);

  function onKeyDown(e: React.KeyboardEvent) {
    if (!open) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
        e.preventDefault();
        setOpen(true);
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, options.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const chosen = options[active];
      if (chosen) {
        onChange(chosen.value);
        setOpen(false);
      }
    }
  }

  return (
    <div className="relative" style={{ width: width ?? "100%" }}>
      <button
        ref={btnRef}
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-label={ariaLabel}
        className="zr-select"
        data-open={open}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={onKeyDown}
      >
        <span className="truncate flex-1 text-left" style={{ color: current ? "var(--text)" : "var(--text-3)" }}>
          {current?.label ?? placeholder ?? "Select\u2026"}
        </span>
        <span style={{ color: "var(--text-3)" }}>
          <ChevronIcon open={open} />
        </span>
      </button>

      <Popover anchor={btnRef} open={open} onDismiss={close} width="anchor" align={align}>
        <div role="listbox">
          {options.map((o, i) => (
            <button
              key={o.value}
              type="button"
              role="option"
              aria-selected={o.value === value}
              className="zr-menu-item"
              data-active={i === active}
              onMouseEnter={() => setActive(i)}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
            >
              <span
                className="shrink-0"
                style={{ width: 12, color: o.value === value ? "var(--accent)" : "transparent" }}
              >
                <CheckIcon />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate">{o.label}</span>
                {o.hint && (
                  <span className="block truncate text-[10.5px]" style={{ color: "var(--text-3)" }}>
                    {o.hint}
                  </span>
                )}
              </span>
            </button>
          ))}
        </div>
      </Popover>
    </div>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: React.ReactNode;
  hint?: React.ReactNode;
  ariaLabel?: string;
}


export function Toggle({ checked, onChange, label, hint, ariaLabel }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)}
      className={`flex gap-2.5 text-left w-full ${hint ? "items-start" : "items-center"}`}
      onClick={() => onChange(!checked)}
    >
      <span
        className="shrink-0 flex items-center"
        style={{ height: hint ? TOGGLE_LINE : undefined }}
      >
        <span className="zr-toggle" data-checked={checked} aria-hidden="true">
          <span className="zr-toggle-knob" />
        </span>
      </span>
      {(label || hint) && (
        <span className="min-w-0 flex-1">
          {label && <span className="block text-[12px] leading-snug">{label}</span>}
          {hint && (
            <span className="block text-[11px] leading-snug mt-0.5" style={{ color: "var(--text-3)" }}>
              {hint}
            </span>
          )}
        </span>
      )}
    </button>
  );
}


export function TextField({
  value, onChange, placeholder, mono, width, invalid, id, ariaLabel, type = "text", min, max,
  padLeft,
}: {
  value: string | number;
  onChange: (v: string) => void;
  placeholder?: string;
  mono?: boolean;
  width?: number | string;
  invalid?: boolean;
  id?: string;
  ariaLabel?: string;
  type?: "text" | "number" | "date";
  min?: number;
  max?: number;

  padLeft?: number;
}) {
  return (
    <input
      id={id}
      aria-label={ariaLabel}
      type={type}
      min={min}
      max={max}
      className={`zr-input ${mono ? "mono" : ""} ${type === "number" ? "tabular-nums" : ""}`}
      style={{
        width: typeof width === "number" ? `${width}px` : (width ?? "100%"),
        paddingLeft: padLeft,
      }}
      data-invalid={invalid || undefined}
      value={value}
      placeholder={placeholder}
      spellCheck={false}
      autoComplete="off"
      onChange={(e) => onChange(e.target.value)}
    />
  );
}


export function Popover({
  anchor,
  open,
  onDismiss,
  children,
  width = 160,
  align = "right",
}: {
  anchor: React.RefObject<HTMLElement | null>;
  open: boolean;
  onDismiss: () => void;
  children: React.ReactNode;

  width?: number | "anchor";
  align?: "left" | "right";
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [minWidth, setMinWidth] = useState<number>();
  const [box, setBox] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    if (!open || !anchor.current) {
      setBox(null);
      setMinWidth(undefined);
      return;
    }
    const a = anchor.current.getBoundingClientRect();


    const wantMin = width === "anchor" ? a.width : undefined;
    if (wantMin !== minWidth) {
      setMinWidth(wantMin);
      return;
    }

    const el = menuRef.current;
    const w = el?.offsetWidth ?? (typeof width === "number" ? width : a.width);
    const h = el?.offsetHeight ?? 0;
    const gap = 4;

    const below = window.innerHeight - a.bottom - gap;
    const openUp = h > below && a.top - gap > below;
    const top = openUp ? Math.max(gap, a.top - gap - h) : a.bottom + gap;

    const left = align === "right" ? a.right - w : a.left;
    setBox({ top, left: Math.max(gap, Math.min(left, window.innerWidth - w - gap)) });
  }, [open, anchor, width, align, children, minWidth]);

  useEffect(() => {
    if (!open) return;

    const onPointerDown = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;


      if (menuRef.current?.contains(t) || anchor.current?.contains(t)) return;
      onDismiss();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onDismiss();
    };

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", onDismiss);
    window.addEventListener("scroll", onDismiss, true);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onDismiss);
      window.removeEventListener("scroll", onDismiss, true);
    };
  }, [open, anchor, onDismiss]);

  if (!open) return null;

  return createPortal(
    <div
      ref={menuRef}
      data-zr-popover=""
      className="zr-menu fade-in"
      style={{
        position: "fixed",
        width: width === "anchor" ? undefined : width,
        minWidth,
        maxWidth: "min(380px, 92vw)",
        top: box?.top ?? -9999,
        left: box?.left ?? -9999,
        visibility: box ? "visible" : "hidden",
      }}
    >
      {children}
    </div>,
    document.body,
  );
}
