import { useEffect, useMemo, useRef, useState } from "react";
import { CornerDownLeft } from "lucide-react";

export interface Command {
  id: string;
  label: string;
  hint?: string;
  shortcut?: string;
  run: () => void;
  disabled?: boolean;
}

interface Props {
  open: boolean;
  onClose: () => void;
  commands: Command[];
}

export function CommandPalette({ open, onClose, commands }: Props) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    const live = commands.filter((c) => !c.disabled);
    if (!q) return live;
    return live.filter((c) => `${c.label} ${c.hint ?? ""}`.toLowerCase().includes(q));
  }, [commands, query]);

  useEffect(() => setActive(0), [query]);

  if (!open) return null;

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, matches.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const chosen = matches[active];
      if (chosen) {
        onClose();
        chosen.run();
      }
    }
  }

  return (
    <div
      className="fixed inset-0 z-40 flex items-start justify-center pt-[14vh]"
      style={{ background: "rgba(0,0,0,0.45)" }}
      onMouseDown={onClose}
    >
      <div
        className="panel w-[520px] max-w-[90vw] overflow-hidden fade-in"
        style={{ background: "var(--raised)", boxShadow: "var(--shadow)" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="zr-input w-full !border-0 !rounded-none !px-3.5 !py-3 text-[13px]"
          style={{ background: "transparent", borderBottom: "1px solid var(--border)" }}
          placeholder="Type a command"
          value={query}
          spellCheck={false}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />

        <div className="max-h-[340px] overflow-y-auto py-1">
          {matches.length === 0 ? (
            <div className="px-3.5 py-3 text-[12px]" style={{ color: "var(--text-3)" }}>
              Nothing matches.
            </div>
          ) : (
            matches.map((c, i) => (
              <button
                key={c.id}
                className="w-full flex items-center gap-3 px-3.5 py-2 text-left"
                style={{ background: i === active ? "var(--row-selected)" : "transparent" }}
                data-active={i === active}
                onMouseEnter={() => setActive(i)}
                onClick={() => {
                  onClose();
                  c.run();
                }}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[12.5px] truncate">{c.label}</div>
                  {c.hint && (
                    <div className="text-[11px] truncate" style={{ color: "var(--text-3)" }}>{c.hint}</div>
                  )}
                </div>
                {c.shortcut && <kbd className="chip mono !text-[10px]">{c.shortcut}</kbd>}
                {i === active && <CornerDownLeft size={12} style={{ color: "var(--text-3)" }} />}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
