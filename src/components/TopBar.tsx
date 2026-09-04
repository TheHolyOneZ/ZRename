import { open } from "@tauri-apps/plugin-dialog";
import { save } from "@tauri-apps/plugin-dialog";
import { ChevronDown, Clock, Command, FolderOpen, RefreshCw, Save } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, describeError } from "../lib/tauri";
import { formatCount, middleEllipsis } from "../lib/format";
import { useRuleStore } from "../store/useRuleStore";
import { useSessionStore } from "../store/useSessionStore";
import { useSettingsStore } from "../store/useSettingsStore";
import { toast } from "../store/useToastStore";
import type { Preset } from "../types";
import { WatchToggle } from "./WatchToggle";
import { CheckIcon, Popover } from "./ui";
import { PromptDialog } from "./PromptDialog";

export function TopBar({ onPalette }: { onPalette: () => void }) {
  const { scan, summary, presetName, setPresetName, load, rescan, busy, setScanOptions } =
    useSessionStore();
  const { rules, setRules } = useRuleStore();
  const [presets, setPresets] = useState<Preset[]>([]);
  const [open_, setOpen] = useState(false);
  const [naming, setNaming] = useState(false);
  const [recentOpen, setRecentOpen] = useState(false);
  const folderBtn = useRef<HTMLButtonElement>(null);
  const closeRecent = useCallback(() => setRecentOpen(false), []);
  const recent = useSettingsStore((s) => s.recentFolders);
  const presetBtn = useRef<HTMLButtonElement>(null);
  const closePresets = useCallback(() => setOpen(false), []);

  useEffect(() => {
    api.listPresets().then(setPresets).catch(() => {});
  }, []);

  useEffect(() => {
    const open = () => savePreset();
    window.addEventListener("zrename:save-preset", open);
    return () => window.removeEventListener("zrename:save-preset", open);
  });

  async function choose() {
    const picked = await open({ multiple: true, directory: true });
    if (!picked) return;
    await load(Array.isArray(picked) ? picked : [picked]);
  }

  function applyPreset(p: Preset) {
    setRules(p.rules.map((r) => ({ ...r, id: r.id || crypto.randomUUID() })));
    setPresetName(p.name);
    if (p.scan) setScanOptions(p.scan);
    setOpen(false);
    toast.info(`Loaded “${p.name}”`, `${p.rules.length} rule${p.rules.length === 1 ? "" : "s"}`);
  }

  function savePreset() {
    if (rules.length === 0) {
      toast.warn("Nothing to save", "Add a rule first.");
      return;
    }
    setNaming(true);
  }

  async function commitPreset(name: string) {
    setNaming(false);
    try {
      await api.savePreset({ name, rules });
      setPresetName(name);
      setPresets(await api.listPresets());
      toast.success(`Saved “${name}”`);
    } catch (e) {
      toast.error("Could not save the preset", describeError(e));
    }
  }

  const root = scan?.roots[0] ?? "";

  return (
    <div
      className="flex items-center gap-3 px-3 h-10 shrink-0"
      style={{ background: "var(--surface-2)", borderBottom: "1px solid var(--border)" }}
    >
      <div className="relative min-w-0 flex items-center">
        <button className="btn btn-ghost !py-1 !px-2 min-w-0" onClick={choose} title={root}>
          <FolderOpen size={13} className="shrink-0" />
          <span className="truncate text-[12px]">
            {root ? middleEllipsis(root, 42) : "Choose a folder"}
          </span>
        </button>

        {recent.length > 0 && (
          <button
            ref={folderBtn}
            className="btn btn-ghost !p-1 shrink-0"
            onClick={() => setRecentOpen((v) => !v)}
            title="Folders you worked on recently"
            aria-label="Recent folders"
          >
            <ChevronDown size={11} />
          </button>
        )}

        <Popover anchor={folderBtn} open={recentOpen} onDismiss={closeRecent} width={420} align="left">
          <div>
            <div className="px-2 py-1 label">Recent</div>
            {recent.map((p) => (
              <button
                key={p}
                className="zr-menu-item"
                onClick={() => {
                  setRecentOpen(false);
                  load([p]);
                }}
              >
                <Clock size={11} className="shrink-0" style={{ color: "var(--text-3)", marginTop: 2 }} />
                <span className="mono truncate flex-1 !text-[11px]" title={p}>
                  {middleEllipsis(p, 58)}
                </span>
              </button>
            ))}
          </div>
        </Popover>
      </div>

      {scan && (
        <span className="text-[11.5px] tabular-nums shrink-0" style={{ color: "var(--text-3)" }}>
          {formatCount(summary.total || scan.total)} file{(summary.total || scan.total) === 1 ? "" : "s"}
          {scan.folders > 0 && ` · ${formatCount(scan.folders)} folder${scan.folders === 1 ? "" : "s"}`}
        </span>
      )}

      {scan && (
        <span
          className="chip shrink-0"
          title={
            scan.caseInsensitive
              ? `${scan.fsName} ignores letter case, so names differing only in case collide.`
              : `${scan.fsName} treats letter case as significant.`
          }
        >
          {scan.fsName}
          {scan.caseInsensitive && " · case-insensitive"}
        </span>
      )}

      <button className="btn btn-ghost !p-1.5 shrink-0" onClick={() => rescan()} disabled={!scan || busy} title="Re-read from disk (F5)">
        <RefreshCw size={13} className={busy ? "animate-spin" : undefined} />
      </button>

      <div className="flex-1" />

      <WatchToggle />

      <button className="btn btn-ghost !py-1 !px-2 shrink-0" onClick={savePreset} title="Save the rule stack as a preset">
        <Save size={13} />
      </button>

      <div className="relative shrink-0">
        <button
          ref={presetBtn}
          className="btn !py-1 !px-2 text-[12px]"
          onClick={() => setOpen((v) => !v)}
        >
          <span style={{ color: "var(--text-3)" }}>preset:</span>
          <span className="max-w-[160px] truncate">{presetName ?? "none"}</span>
          <ChevronDown size={12} />
        </button>

        <Popover anchor={presetBtn} open={open_} onDismiss={closePresets} width={290}>
          <div>
            {presets.length === 0 ? (
              <div className="px-3 py-2 text-[11.5px]" style={{ color: "var(--text-3)" }}>
                No presets yet.
              </div>
            ) : (
              presets.map((p) => (
                <button key={p.name} className="zr-menu-item" onClick={() => applyPreset(p)}>
                  <span
                    className="shrink-0"
                    style={{ width: 12, color: p.name === presetName ? "var(--accent)" : "transparent" }}
                  >
                    <CheckIcon />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{p.name}</span>
                    {p.description && (
                      <span className="block text-[10.5px] leading-snug" style={{ color: "var(--text-3)" }}>
                        {p.description}
                      </span>
                    )}
                  </span>
                </button>
              ))
            )}

            <div style={{ borderTop: "1px solid var(--border)" }} className="mt-1 pt-1">
              <button
                className="zr-menu-item"
                onClick={async () => {
                  setOpen(false);
                  const p = await open({ multiple: false, filters: [{ name: "Preset", extensions: ["toml"] }] });
                  if (typeof p !== "string") return;
                  try {
                    const imported = await api.importPreset(p);
                    setPresets(await api.listPresets());
                    applyPreset(imported);
                  } catch (e) {
                    toast.error("Could not import that preset", describeError(e));
                  }
                }}
              >
                Import a preset…
              </button>
              <button
                className="zr-menu-item"
                disabled={rules.length === 0}
                onClick={async () => {
                  setOpen(false);
                  const path = await save({
                    defaultPath: `${(presetName ?? "preset").replace(/\W+/g, "-").toLowerCase()}.toml`,
                    filters: [{ name: "Preset", extensions: ["toml"] }],
                  });
                  if (!path) return;
                  try {
                    await api.exportPreset({ name: presetName ?? "Exported", rules }, path);
                    toast.success("Exported the preset", path);
                  } catch (e) {
                    toast.error("Could not export", describeError(e));
                  }
                }}
              >
                Export the current stack…
              </button>
            </div>
          </div>
        </Popover>
      </div>

      <button className="btn btn-ghost !py-1 !px-2 shrink-0" onClick={onPalette} title="Command palette (Ctrl+K)">
        <Command size={12} />
        <span className="text-[11px]">K</span>
      </button>

      <PromptDialog
        open={naming}
        title="Save preset"
        label="Name"
        placeholder="Photos \u2192 date-based"
        hint={`Saved as TOML in the presets folder, alongside the ${rules.length} rule${rules.length === 1 ? "" : "s"} in the stack.`}
        initial={presetName ?? "My preset"}
        onCancel={() => setNaming(false)}
        onConfirm={commitPreset}
      />
    </div>
  );
}
