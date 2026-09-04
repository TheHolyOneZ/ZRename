import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { useSessionStore } from "../store/useSessionStore";
import { toast } from "../store/useToastStore";
import { describeError } from "../lib/tauri";
import { Modal } from "./Modal";


function visit(url: string) {
  openUrl(url).catch((e) =>
    toast.error("Could not open that link", `${url} — ${describeError(e)}`),
  );
}

const LINKS: { url: string; label: string; hint: string }[] = [
  {
    url: "https://zsync.eu/zrename/",
    label: "zsync.eu/zrename",
    hint: "Homepage and downloads",
  },
  {
    url: "https://github.com/TheHolyOneZ/ZRename",
    label: "ZRename on GitHub",
    hint: "Source code, releases and issues",
  },
  {
    url: "https://github.com/TheHolyOneZ",
    label: "TheHolyOneZ on GitHub",
    hint: "The author",
  },
  { url: "https://zsync.eu", label: "zsync.eu", hint: "More projects like this one" },
  { url: "https://zlogic.eu", label: "zlogic.eu", hint: "Game mods" },
];

const SHORTCUTS: [string, string][] = [
  ["Ctrl+K", "Command palette"],
  ["Ctrl+Enter", "Apply"],
  ["Ctrl+Z", "Undo the last batch"],
  ["Alt+↑ / ↓", "Reorder the selected rule"],
  ["Ctrl+D", "Duplicate the selected rule"],
  ["Ctrl+N", "Add a rule"],
  ["Ctrl+S", "Save the stack as a preset"],
  ["Delete", "Remove the selected rule"],
  ["Space", "Toggle the selected rule"],
  ["Ctrl+F", "Filter the preview"],
  ["F5", "Re-read the folder"],
];

export function About({ open, onClose }: { open: boolean; onClose: () => void }) {
  const caps = useSessionStore((s) => s.caps);

  return (
    <Modal open={open} onClose={onClose} title="About ZRename" width={460}>
      <div className="flex flex-col gap-4">
        <div className="flex items-center gap-3">
          <img src="/icon.png" alt="" className="w-12 h-12" />
          <div>
            <div className="text-[14px] font-semibold">ZRename {caps?.version && `v${caps.version}`}</div>
            <div className="text-[11.5px]" style={{ color: "var(--text-3)" }}>
              A rule pipeline for renaming ten thousand files without fear
            </div>
          </div>
        </div>

        <p className="text-[12px] leading-relaxed" style={{ color: "var(--text-2)" }}>
          Every batch is journalled before the first file moves, so undo works
          even after a restart — and it verifies each file before putting it
          back, rather than overwriting whatever is there now.
        </p>

        <div className="flex flex-col gap-1">
          <span className="label">Keyboard</span>
          <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            {SHORTCUTS.map(([key, what]) => (
              <div key={key} className="contents">
                <kbd className="chip mono !text-[10px] justify-self-start">{key}</kbd>
                <span className="text-[11.5px]" style={{ color: "var(--text-2)" }}>{what}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-1 pt-3" style={{ borderTop: "1px solid var(--border)" }}>
          <span className="label">Elsewhere</span>
          {LINKS.map((l) => (
            <button
              key={l.url}
              className="zr-menu-item !px-2"
              onClick={() => visit(l.url)}
            >
              <ExternalLink size={12} className="shrink-0" style={{ color: "var(--text-3)", marginTop: 2 }} />
              <span className="min-w-0 flex-1">
                <span className="block truncate">{l.label}</span>
                <span className="block truncate text-[10.5px]" style={{ color: "var(--text-3)" }}>
                  {l.hint}
                </span>
              </span>
            </button>
          ))}
        </div>

        <div className="pt-3" style={{ borderTop: "1px solid var(--border)" }}>
          <span className="text-[11px]" style={{ color: "var(--text-3)" }}>
            GPL-3.0 · TheHolyOneZ
          </span>
        </div>
      </div>
    </Modal>
  );
}
