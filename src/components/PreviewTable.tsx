import { useVirtualizer } from "@tanstack/react-virtual";
import {
  AlertTriangle, ArrowRight, Ban, Check, CircleSlash, Folder, GripVertical,
  Link2, Minus, Ruler, Search, TableProperties,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { edgeBounds, sideSpans, splitEdges } from "../lib/format";
import { api } from "../lib/tauri";
import { rowHeight, useSettingsStore } from "../store/useSettingsStore";
import { usePreviewStore, rowAt } from "../store/usePreviewStore";
import { useSessionStore } from "../store/useSessionStore";
import { toast } from "../store/useToastStore";
import type { DiffSpan, Row, RowStatusKey } from "../types";
import { Checkbox, TextField } from "./ui";

const STATUS_ICON: Record<RowStatusKey, typeof Check> = {
  ok: Check,
  unchanged: Minus,
  collision: Ban,
  invalid: AlertTriangle,
  too_long: Ruler,
  reserved_name: CircleSlash,
  skipped: Minus,
};

function statusColor(status: RowStatusKey): string {
  switch (status) {

    case "collision": return "var(--collision)";
    case "invalid":
    case "too_long":
    case "reserved_name": return "var(--warn)";
    case "ok": return "var(--ok)";
    default: return "var(--text-3)";
  }
}


const DRAG_ROW_LIMIT = 20000;

export function PreviewTable() {
  const settings = useSettingsStore();
  const preview = usePreviewStore();
  const summary = useSessionStore((s) => s.summary);

  const filters = useMemo(
    () => ({ hideUnchanged: settings.hideUnchanged, collisionsFirst: settings.collisionsFirst }),
    [settings.hideUnchanged, settings.collisionsFirst],
  );

  const parentRef = useRef<HTMLDivElement>(null);
  const height = rowHeight(settings.comfortable);

  const virtualizer = useVirtualizer({
    count: preview.total,
    getScrollElement: () => parentRef.current,
    estimateSize: () => height,
    overscan: 12,
  });

  useEffect(() => {
    preview.invalidate(filters);

  }, [summary, filters]);

  const items = virtualizer.getVirtualItems();

  useEffect(() => {
    if (items.length === 0) return;
    preview.ensure(items[0].index, items[items.length - 1].index, filters);

  }, [items, filters]);

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <Header />
      <ColumnHeader />

      <div ref={parentRef} className="flex-1 overflow-auto min-h-0" tabIndex={0}>
        {preview.total === 0 ? (
          <div className="h-full flex items-center justify-center text-[12px]" style={{ color: "var(--text-3)" }}>
            {summary.total === 0
              ? "Nothing loaded."
              : preview.search || preview.onlyProblems || settings.hideUnchanged
                ? "No rows match this filter."
                : "No files."}
          </div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative", width: "100%" }}>
            {items.map((item) => {
              const row = rowAt(preview, item.index);
              return (
                <div
                  key={item.key}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: item.size,
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  {row ? (
                    <PreviewRow row={row} selected={preview.selected === row.index} height={height} />
                  ) : (
                    <div className="h-full flex items-center px-3">
                      <div className="h-[9px] w-1/3 rounded" style={{ background: "var(--border)" }} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function Header() {
  const settings = useSettingsStore();
  const preview = usePreviewStore();
  const summary = useSessionStore((s) => s.summary);
  const clearExclusions = useSessionStore((s) => s.clearExclusions);
  const filters = { hideUnchanged: settings.hideUnchanged, collisionsFirst: settings.collisionsFirst };


  const csv = useRef<string | null>(null);
  const [preparing, setPreparing] = useState(false);

  async function prepare() {
    if (csv.current !== null || preparing || summary.total === 0) return;
    if (summary.total > DRAG_ROW_LIMIT) return;
    setPreparing(true);
    try {
      csv.current = await api.exportPlan("csv");
    } catch {
      csv.current = null;
    } finally {
      setPreparing(false);
    }
  }

  useEffect(() => {
    csv.current = null;
  }, [summary]);

  const tooBig = summary.total > DRAG_ROW_LIMIT;

  return (
    <div className="flex items-center gap-2 px-3 py-2 shrink-0">
      <span className="label">Preview</span>

      <div className="relative flex-1 max-w-[260px]">
        <Search size={12} className="absolute left-2 top-1/2 -translate-y-1/2 pointer-events-none z-10" style={{ color: "var(--text-3)" }} />
        <TextField
          id="preview-filter"
          ariaLabel="Filter rows"
          placeholder="Filter rows"
          padLeft={26}
          value={preview.search}
          onChange={(v) => preview.setSearch(v, filters)}
        />
      </div>

      <div className="flex-1" />

      {summary.skipped > 0 && (
        <button
          className="btn btn-ghost !py-0.5 !px-1.5 text-[11px]"
          onClick={clearExclusions}
          title="Tick every row again"
        >
          restore {summary.skipped}
        </button>
      )}

      <button
        className="btn btn-ghost !p-1"
        draggable={!tooBig}
        onMouseEnter={prepare}
        onDragStart={(e) => {
          if (!csv.current) {
            e.preventDefault();
            toast.info("Still preparing the table", "Try the drag again in a moment.");
            return;
          }
          e.dataTransfer.setData("text/csv", csv.current);
          e.dataTransfer.setData("text/plain", csv.current);
          e.dataTransfer.effectAllowed = "copy";
        }}
        title={
          tooBig
            ? "Too many rows to drag; use Dry run to write a file"
            : "Drag into a spreadsheet or editor. Dry run writes it to a file."
        }
        style={{ opacity: tooBig ? 0.4 : 1, cursor: tooBig ? "not-allowed" : "grab" }}
        aria-label="Drag the table out as CSV"
      >
        <TableProperties size={13} />
      </button>

      <Checkbox
        checked={preview.onlyProblems}
        onChange={(v) => preview.setOnlyProblems(v, filters)}
        label="only problems"
      />

      <Checkbox
        checked={settings.hideUnchanged}
        onChange={(v) => settings.set("hideUnchanged", v)}
        label="hide unchanged"
      />
    </div>
  );
}


function ColumnHeader() {
  const { columnSplit, set } = useSettingsStore();
  const ref = useRef<HTMLDivElement>(null);

  function startDrag(e: React.PointerEvent<HTMLButtonElement>) {
    e.preventDefault();
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;

    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);

    const move = (ev: PointerEvent) => {
      const ratio = (ev.clientX - box.left) / box.width;
      set("columnSplit", Math.min(0.85, Math.max(0.15, ratio)));
    };
    const up = () => {
      handle.releasePointerCapture(e.pointerId);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
  }

  return (
    <div
      ref={ref}
      className="flex items-center px-3 py-1 shrink-0 text-[10px]"
      style={{ color: "var(--text-3)", borderBottom: "1px solid var(--border)" }}
    >
      <span className="w-[18px] shrink-0" />
      <span className="uppercase tracking-[0.09em] font-semibold truncate" style={{ flex: columnSplit }}>
        old
      </span>

      <button
        className="w-3 shrink-0 flex items-center justify-center self-stretch mx-2"
        style={{ cursor: "col-resize" }}
        onPointerDown={startDrag}
        onDoubleClick={() => set("columnSplit", 0.5)}
        title="Drag to resize, double-click to even up"
        aria-label="Resize the columns"
      >
        <GripVertical size={9} style={{ opacity: 0.5 }} />
      </button>

      <span
        className="uppercase tracking-[0.09em] font-semibold truncate"
        style={{ flex: 1 - columnSplit }}
      >
        new
      </span>
      <span className="w-[18px] text-center uppercase tracking-[0.09em] font-semibold shrink-0">st</span>
    </div>
  );
}

function PreviewRow({ row, selected, height }: { row: Row; selected: boolean; height: number }) {
  const select = usePreviewStore((s) => s.select);
  const setRowExcluded = useSessionStore((s) => s.setRowExcluded);
  const columnSplit = useSettingsStore((s) => s.columnSplit);
  const Icon = STATUS_ICON[row.status];
  const color = statusColor(row.status);

  return (
    <div
      className="flex items-center px-3 h-full cursor-default"
      style={{ background: selected ? "var(--row-selected)" : undefined, opacity: row.excluded ? 0.45 : 1 }}
      onMouseEnter={(e) => {
        if (!selected) e.currentTarget.style.background = "var(--row-hover)";
      }}
      onMouseLeave={(e) => {
        if (!selected) e.currentTarget.style.background = "";
      }}
      onClick={() => select(row)}
      title={`${row.fromPath}\n→ ${row.toPath}\n${row.statusLabel}`}
    >
      <span className="w-[18px] shrink-0 flex">
        <Checkbox
          checked={!row.excluded}
          onChange={(v) => setRowExcluded(row.index, !v)}
          stopPropagation
          ariaLabel={row.excluded ? `Include ${row.fromName}` : `Leave ${row.fromName} alone`}
        />
      </span>

      <div className="min-w-0 flex items-center gap-1" style={{ flex: columnSplit }}>
        {row.isDir && <Folder size={11} className="shrink-0" style={{ color: "var(--text-3)" }} />}
        {row.isSymlink && (
          <Link2
            size={11}
            className="shrink-0"
            style={{ color: "var(--text-3)" }}
            aria-label="Symbolic link"
          />
        )}
        <NameCell spans={row.diff} side="old" />
      </div>

      <span className="w-3 shrink-0 flex justify-center mx-2">
        <ArrowRight size={11} style={{ color: "var(--text-3)" }} />
      </span>

      <div className="min-w-0 flex items-center gap-1" style={{ flex: 1 - columnSplit }}>
        {row.status === "skipped" ? (
          <span className="mono truncate" style={{ color: "var(--text-3)" }}>
            (skipped)
          </span>
        ) : (
          <NameCell spans={row.diff} side="new" dim={row.status === "unchanged"} />
        )}
        {row.moved && <span className="chip shrink-0 !text-[9.5px]" title={row.toPath}>moved</span>}
        {row.caseOnly && (
          <span className="chip shrink-0 !text-[9.5px]" title="Only the letter case changes; this needs a two-phase rename.">
            case
          </span>
        )}
      </div>

      <div className="w-[18px] flex items-center justify-center shrink-0" style={{ color, height }}>
        <Icon size={12} strokeWidth={row.status === "collision" ? 2.5 : 2} />
      </div>
    </div>
  );
}


function NameCell({ spans, side, dim }: { spans: DiffSpan[]; side: "old" | "new"; dim?: boolean }) {
  const visible = sideSpans(spans, side);
  const full = visible.map((s) => s.text).join("");
  const bounds = edgeBounds(full);

  let offset = 0;
  return (
    <span className="mono truncate" style={dim ? { color: "var(--text-3)" } : undefined}>
      {visible.map((span, i) => {
        const parts = splitEdges(span.text, offset, bounds);
        offset += span.text.length;
        const cls = span.op === "insert" ? "diff-add" : span.op === "delete" ? "diff-del" : "";
        return (
          <span key={i} className={cls}>
            {parts.map((p, j) =>
              p.ws ? (
                <span key={j} className="ws-dot">{"·".repeat(p.text.length)}</span>
              ) : (
                <span key={j}>{p.text}</span>
              ),
            )}
          </span>
        );
      })}
    </span>
  );
}
