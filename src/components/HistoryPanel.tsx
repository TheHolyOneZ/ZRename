import { ChevronDown, ChevronRight, Undo2 } from "lucide-react";
import { useState } from "react";
import { formatBatchTime } from "../lib/format";
import { useSessionStore } from "../store/useSessionStore";

export function HistoryPanel() {
  const { history, undo, busy } = useSessionStore();
  const [open, setOpen] = useState(true);

  return (
    <div className="shrink-0 flex flex-col min-h-0" style={{ borderTop: "1px solid var(--border)" }}>
      <button className="w-full flex items-center gap-1 px-3 py-2 shrink-0" onClick={() => setOpen((v) => !v)}>
        {open ? <ChevronDown size={12} style={{ color: "var(--text-3)" }} /> : <ChevronRight size={12} style={{ color: "var(--text-3)" }} />}
        <span className="label">History</span>
        {history.length > 0 && (
          <span className="text-[10px] tabular-nums" style={{ color: "var(--text-3)" }}>
            {history.length}
          </span>
        )}
      </button>

      {open && (
        <div className="px-2 pb-2 overflow-y-auto max-h-[180px]">
          {history.length === 0 ? (
            <p className="px-1 py-1 text-[11px] leading-snug" style={{ color: "var(--text-3)" }}>
              Batches you apply appear here, and stay after a restart.
            </p>
          ) : (
            <div className="flex flex-col gap-0.5">
              {history.map((h) => (
                <div
                  key={h.id}
                  className="group flex items-center gap-2 px-1.5 py-1 rounded transition-colors"
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--row-hover)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "")}
                  title={`${h.created}\n${h.roots.join("\n")}`}
                >
                  <span className="text-[11px] tabular-nums shrink-0" style={{ color: "var(--text-2)" }}>
                    {formatBatchTime(h.id)}
                  </span>
                  <span className="text-[11px] truncate flex-1" style={{ color: "var(--text-3)" }}>
                    {h.count} file{h.count === 1 ? "" : "s"}
                    {h.preset ? ` · ${h.preset}` : ""}
                  </span>
                  <button
                    className="btn btn-ghost !p-1 opacity-0 group-hover:opacity-100 focus:opacity-100 shrink-0"
                    disabled={busy}
                    onClick={() => undo(h.id)}
                    title="Put this batch back"
                    aria-label={`Undo the batch from ${formatBatchTime(h.id)}`}
                  >
                    <Undo2 size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
