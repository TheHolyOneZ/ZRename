import { useEffect, useState } from "react";
import { Copy, Loader2 } from "lucide-react";
import { useSessionStore } from "../store/useSessionStore";
import { api, describeError } from "../lib/tauri";
import { formatBytes } from "../lib/format";
import { toast } from "../store/useToastStore";
import type { DupeGroup } from "../types";
import { Modal } from "./Modal";


export function Duplicates({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [groups, setGroups] = useState<DupeGroup[] | null>(null);
  const [busy, setBusy] = useState(false);
  const excludeRows = useSessionStore((s) => s.excludeRows);

  useEffect(() => {
    if (!open) return;
    setBusy(true);
    setGroups(null);
    api
      .findDupes()
      .then(setGroups)
      .catch((e) => toast.error("Could not scan for duplicates", describeError(e)))
      .finally(() => setBusy(false));
  }, [open]);

  const wasted = (groups ?? []).reduce((n, g) => n + g.size * (g.names.length - 1), 0);

  return (
    <Modal open={open} onClose={onClose} title="Duplicate content" width={560}>
      {busy ? (
        <div className="flex items-center gap-2 py-6 justify-center text-[12px]" style={{ color: "var(--text-3)" }}>
          <Loader2 size={14} className="animate-spin" />
          Hashing candidates…
        </div>
      ) : !groups || groups.length === 0 ? (
        <p className="text-[12px] py-4 text-center" style={{ color: "var(--text-3)" }}>
          No two files in this selection have identical content.
        </p>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="flex items-start gap-2">
            <p className="text-[11.5px] flex-1" style={{ color: "var(--text-2)" }}>
              {groups.length} group{groups.length === 1 ? "" : "s"} · {formatBytes(wasted)} held in
              copies. Nothing is deleted here — unticking only leaves those files
              out of the rename.
            </p>
            <button
              className="btn shrink-0"
              onClick={() => {

                const copies = groups.flatMap((g) => g.indices.slice(1));
                excludeRows(copies, true);
                onClose();
              }}
            >
              Untick the copies
            </button>
          </div>

          <div className="flex flex-col gap-2 max-h-[420px] overflow-y-auto">
            {groups.map((g) => (
              <div key={g.hash} className="panel px-2.5 py-2" style={{ background: "var(--surface-2)" }}>
                <div className="flex items-center gap-2 mb-1">
                  <Copy size={11} style={{ color: "var(--text-3)" }} />
                  <span className="text-[11px]" style={{ color: "var(--text-2)" }}>
                    {g.names.length} copies · {formatBytes(g.size)} each
                  </span>
                  <span className="mono text-[10px]" style={{ color: "var(--text-3)" }}>{g.hash}</span>
                </div>
                <div className="flex flex-col gap-0.5 ml-[19px]">
                  {g.names.map((n, i) => (
                    <span key={i} className="mono text-[11px] truncate" title={g.paths[i]}>
                      {n}
                      {i === 0 && (
                        <span className="ml-1.5 !text-[10px]" style={{ color: "var(--text-3)" }}>
                          kept
                        </span>
                      )}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </Modal>
  );
}
