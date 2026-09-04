import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Inbox } from "lucide-react";
import { useSessionStore } from "../store/useSessionStore";


export function DropZone() {
  const load = useSessionStore((s) => s.load);

  async function choose(directory: boolean) {
    const picked = await open({ multiple: true, directory });
    if (!picked) return;
    await load(Array.isArray(picked) ? picked : [picked]);
  }

  return (
    <div className="flex-1 flex items-center justify-center p-10">
      <div className="flex flex-col items-center gap-5 text-center max-w-[440px]">
        <div
          className="w-16 h-16 rounded-2xl flex items-center justify-center"
          style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
        >
          <Inbox size={28} />
        </div>

        <div>
          <div className="text-[15px] font-semibold mb-1.5">Drop files or a folder here</div>
          <p className="text-[12.5px] leading-relaxed" style={{ color: "var(--text-2)" }}>
            Stack up rules, watch the before/after table, then apply. If it was
            wrong, undo puts every file back — including after a restart.
          </p>
        </div>

        <div className="flex gap-2">
          <button className="btn" onClick={() => choose(true)}>
            <FolderOpen size={13} />
            Choose a folder
          </button>
          <button className="btn" onClick={() => choose(false)}>
            Choose files
          </button>
        </div>
      </div>
    </div>
  );
}
