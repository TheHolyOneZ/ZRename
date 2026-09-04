import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { CheckCircle2, FileDown, Loader2, Undo2 } from "lucide-react";
import { api, describeError } from "../lib/tauri";
import { useSessionStore } from "../store/useSessionStore";
import { useRuleStore } from "../store/useRuleStore";
import { useSettingsStore } from "../store/useSettingsStore";
import { AlertTriangle } from "lucide-react";
import { toast } from "../store/useToastStore";
import { formatCount } from "../lib/format";
import { useLayoutEffect, useRef } from "react";


export function CommitBar() {
  const { summary, apply, undo, busy, planning, lastApply, dismissConfirmation } = useSessionStore();
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const h = ref.current?.offsetHeight ?? 0;
    document.documentElement.style.setProperty("--toast-bottom", `${h + 12}px`);
    return () => {
      document.documentElement.style.removeProperty("--toast-bottom");
    };
  });

  if (lastApply) {
    return (
      <div
        ref={ref}
        className="commit-bar flex items-center gap-3 px-3 py-2 shrink-0"
        data-confirmed="true"
      >
        <CheckCircle2 size={15} style={{ color: "var(--ok)" }} className="shrink-0" />
        <span className="text-[12.5px] flex-1">
          Renamed {formatCount(lastApply.renamed)} file{lastApply.renamed === 1 ? "" : "s"}
          {lastApply.twoPhase > 0 && (
            <span style={{ color: "var(--text-3)" }}>
              {" "}· {lastApply.twoPhase} needed a two-phase rename
            </span>
          )}
          {!lastApply.clean && (
            <span style={{ color: "var(--warn)" }}> · some needed attention</span>
          )}
        </span>
        <button className="btn" onClick={dismissConfirmation}>Dismiss</button>
        <button className="btn btn-accent" disabled={busy} onClick={() => undo(lastApply.journalId)}>
          <Undo2 size={13} />
          Undo
        </button>
      </div>
    );
  }

  const blocked = summary.blocking > 0;

  return (
    <div ref={ref} className="shrink-0">
      <FilesystemNotice />
      <div className="commit-bar flex items-center gap-3 px-3 py-2">
      <div className="flex-1 min-w-0">
        <div className="text-[12.5px] truncate flex items-center gap-2">
          {planning && <Loader2 size={12} className="animate-spin shrink-0" style={{ color: "var(--text-3)" }} />}
          <span>{summary.summaryLine}</span>
        </div>
        {blocked && (
          <div className="text-[11px] mt-0.5" style={{ color: "var(--collision)" }}>
            {reasonText(summary)} Resolve them, or change what happens when a name is taken.
          </div>
        )}
      </div>

      <button className="btn" onClick={dryRun} disabled={summary.total === 0}>
        <FileDown size={13} />
        Dry run
      </button>

      <button
        className="btn btn-accent"
        disabled={!summary.canApply || busy}
        onClick={apply}
        title={blocked ? "Resolve the rows above first" : undefined}
      >
        {busy && <Loader2 size={13} className="animate-spin" />}
        {summary.applyLabel}
      </button>
      </div>
    </div>
  );
}


function FilesystemNotice() {
  const scan = useSessionStore((s) => s.scan);
  const rules = useRuleStore((s) => s.rules);
  const dismissed = useSettingsStore((s) => s.dismissedFsNotice);
  const set = useSettingsStore((s) => s.set);

  if (!scan || !scan.needsSanitising || dismissed) return null;

  const sanitising = rules.some((r) => r.enabled && r.kind === "sanitise");
  if (sanitising) return null;

  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 text-[11.5px]"
      style={{ background: "var(--accent-soft)", borderTop: "1px solid var(--border)" }}
    >
      <AlertTriangle size={12} className="shrink-0" style={{ color: "var(--warn)" }} />
      <span className="flex-1 min-w-0">
        <b>{scan.fsName}</b> ignores letter case and rejects
        <code className="mono"> {"< > : \" / \\ | ? *"}</code>, and names are capped at{" "}
        {scan.maxPath ?? 260} characters. Add a Sanitise rule, or load the
        “Sanitise for USB/FAT32” preset.
      </span>
      <button
        className="btn btn-ghost !py-0.5 !px-1.5 text-[11px] shrink-0"
        onClick={() => set("dismissedFsNotice", true)}
      >
        Dismiss
      </button>
    </div>
  );
}

function reasonText(s: { collisions: number; invalid: number; tooLong: number; reserved: number }): string {
  const bits: string[] = [];
  if (s.collisions) bits.push(`${s.collisions} collision${s.collisions === 1 ? "" : "s"}`);
  if (s.invalid) bits.push(`${s.invalid} invalid name${s.invalid === 1 ? "" : "s"}`);
  if (s.tooLong) bits.push(`${s.tooLong} too long`);
  if (s.reserved) bits.push(`${s.reserved} reserved`);
  return `${bits.join(", ")} blocking Apply.`;
}

async function dryRun() {
  try {
    const path = await save({
      title: "Save the planned changes",
      defaultPath: "zrename-plan.csv",
      filters: [
        { name: "CSV", extensions: ["csv"] },
        { name: "Markdown", extensions: ["md"] },
      ],
    });
    if (!path) return;
    const format = path.toLowerCase().endsWith(".md") ? "markdown" : "csv";
    await writeTextFile(path, await api.exportPlan(format));
    toast.success("Wrote the plan", path);
  } catch (e) {
    toast.error("Could not write the plan", describeError(e));
  }
}
