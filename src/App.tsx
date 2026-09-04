import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useMemo, useState } from "react";
import { api } from "./lib/tauri";
import type { Preset } from "./types";
import { About } from "./components/About";
import { CommandPalette, type Command } from "./components/CommandPalette";
import { CommitBar } from "./components/CommitBar";
import { DropZone } from "./components/DropZone";
import { Duplicates } from "./components/Duplicates";
import { FilterPanel } from "./components/FilterPanel";
import { HistoryPanel } from "./components/HistoryPanel";
import { PreviewTable } from "./components/PreviewTable";
import { RuleEditor } from "./components/RuleEditor";
import { RuleStack } from "./components/RuleStack";
import { Settings } from "./components/Settings";
import { Titlebar } from "./components/Titlebar";
import { Toasts } from "./components/Toast";
import { TopBar } from "./components/TopBar";
import { RULE_LABELS, RULE_ORDER } from "./lib/rules";
import { middleEllipsis } from "./lib/format";
import { useRuleStore } from "./store/useRuleStore";
import { useSessionStore } from "./store/useSessionStore";
import { useSettingsStore } from "./store/useSettingsStore";
import { THEMES } from "./types";

export default function App() {
  const session = useSessionStore();
  const rules = useRuleStore((s) => s.rules);
  const selectedId = useRuleStore((s) => s.selectedId);
  const settings = useSettingsStore();

  const [palette, setPalette] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [about, setAbout] = useState(false);
  const [dupes, setDupes] = useState(false);

  const selected = rules.find((r) => r.id === selectedId) ?? null;

  useEffect(() => {
    session.init();

  }, []);


  useEffect(() => {
    if (session.scan) session.replan(rules);

  }, [rules, session.scan]);


  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "drop" && e.payload.paths.length > 0) {
        session.load(e.payload.paths);
      }
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };

  }, []);

  const [presets, setPresets] = useState<Preset[]>([]);
  useEffect(() => {
    api.listPresets().then(setPresets).catch(() => {});
  }, []);

  const commands = useCommands(setSettingsOpen, setAbout, setDupes, presets);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const typing =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable === true;


      const onControl = target?.closest("button, [role='listbox']") != null;

      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((v) => !v);
        return;
      }
      if (ctrl && e.key === "Enter") {
        e.preventDefault();
        session.apply();
        return;
      }
      if (ctrl && e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        session.undo(null);
        return;
      }
      if (ctrl && e.key.toLowerCase() === "f") {
        e.preventDefault();
        document.getElementById("preview-filter")?.focus();
        return;
      }
      if (e.key === "F5") {
        e.preventDefault();
        session.rescan();
        return;
      }
      if (ctrl && e.key.toLowerCase() === "s") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("zrename:save-preset"));
        return;
      }
      if (ctrl && e.key.toLowerCase() === "n") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("zrename:add-rule"));
        return;
      }
      if (typing) return;

      if (ctrl && e.key.toLowerCase() === "d" && selectedId) {
        e.preventDefault();
        useRuleStore.getState().duplicate(selectedId);
        return;
      }
      if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && selectedId) {
        e.preventDefault();
        useRuleStore.getState().nudge(selectedId, e.key === "ArrowUp" ? -1 : 1);
        return;
      }
      if ((e.key === "Delete" || e.key === "Backspace") && selectedId && !onControl) {
        e.preventDefault();
        useRuleStore.getState().remove(selectedId);
        return;
      }
      if (e.key === " " && selectedId && !onControl) {
        e.preventDefault();
        useRuleStore.getState().toggle(selectedId);
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);

  }, [selectedId]);

  const subtitle = session.scan ? middleEllipsis(session.scan.roots[0] ?? "", 52) : undefined;

  return (
    <div className="h-full flex flex-col" style={{ background: "var(--bg)" }}>
      <Titlebar onAbout={() => setAbout(true)} onSettings={() => setSettingsOpen(true)} subtitle={subtitle} />
      <TopBar onPalette={() => setPalette(true)} />

      {!session.scan ? (
        <DropZone />
      ) : (
        <div className="flex-1 flex min-h-0">
          <aside
            className="w-[300px] shrink-0 flex flex-col min-h-0 overflow-y-auto"
            style={{ background: "var(--surface-2)", borderRight: "1px solid var(--border)" }}
          >
            <RuleStack />
            <div className="flex-1" />
            <FilterPanel />
            <HistoryPanel />
          </aside>

          <main className="flex-1 flex flex-col min-w-0">
            <PreviewTable />

            {selected && (
              <div
                className="shrink-0 max-h-[42%] overflow-y-auto"
                style={{ borderTop: "1px solid var(--border)", background: "var(--surface-2)" }}
              >
                <RuleEditor rule={selected} />
              </div>
            )}

            <CommitBar />
          </main>
        </div>
      )}

      <CommandPalette open={palette} onClose={() => setPalette(false)} commands={commands} />
      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <About open={about} onClose={() => setAbout(false)} />
      <Duplicates open={dupes} onClose={() => setDupes(false)} />
      <Toasts />


      <span hidden>{settings.theme}</span>
    </div>
  );
}

function useCommands(
  openSettings: (v: boolean) => void,
  openAbout: (v: boolean) => void,
  openDupes: (v: boolean) => void,
  presets: Preset[],
): Command[] {
  const session = useSessionStore();
  const settings = useSettingsStore();

  return useMemo(() => {
    const list: Command[] = [
      {
        id: "apply",
        label: session.summary.applyLabel,
        hint: session.summary.summaryLine,
        shortcut: "Ctrl+Enter",
        run: () => session.apply(),
        disabled: !session.summary.canApply,
      },
      {
        id: "undo",
        label: "Undo the last batch",
        hint: session.history[0] ? `${session.history[0].count} files` : undefined,
        shortcut: "Ctrl+Z",
        run: () => session.undo(null),
        disabled: session.history.length === 0,
      },
      { id: "rescan", label: "Re-read the folder", shortcut: "F5", run: () => session.rescan() },
      {
        id: "hide-unchanged",
        label: settings.hideUnchanged ? "Show unchanged rows" : "Hide unchanged rows",
        run: () => settings.set("hideUnchanged", !settings.hideUnchanged),
      },
      {
        id: "density",
        label: settings.comfortable ? "Use compact rows" : "Use comfortable rows",
        run: () => settings.set("comfortable", !settings.comfortable),
      },
      {
        id: "dupes",
        label: "Find duplicate content",
        hint: "Group files with identical content",
        run: () => openDupes(true),
        disabled: !session.scan,
      },
      { id: "settings", label: "Settings", run: () => openSettings(true) },
      { id: "about", label: "About ZRename", run: () => openAbout(true) },
    ];

    for (const p of presets) {
      list.push({
        id: `preset-${p.name}`,
        label: `Preset: ${p.name}`,
        hint: p.description ?? `${p.rules.length} rules`,
        run: () => {
          useRuleStore.getState().setRules(
            p.rules.map((r) => ({ ...r, id: r.id || crypto.randomUUID() })),
          );
          session.setPresetName(p.name);
          if (p.scan) session.setScanOptions(p.scan);
        },
      });
    }

    for (const kind of RULE_ORDER) {
      list.push({
        id: `add-${kind}`,
        label: `Add rule: ${RULE_LABELS[kind]}`,
        run: () => useRuleStore.getState().add(kind),
      });
    }

    for (const t of THEMES) {
      list.push({
        id: `theme-${t.id}`,
        label: `Theme: ${t.label}`,
        run: () => settings.setTheme(t.id),
        disabled: settings.theme === t.id,
      });
    }

    return list;
  }, [session, settings, openSettings, openAbout, openDupes, presets]);
}
