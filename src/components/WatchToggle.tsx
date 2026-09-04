import { listen } from "@tauri-apps/api/event";
import { Eye, EyeOff } from "lucide-react";
import { useEffect, useState } from "react";
import { api, describeError } from "../lib/tauri";
import { useSessionStore } from "../store/useSessionStore";
import { toast } from "../store/useToastStore";
import { useRuleStore } from "../store/useRuleStore";
import type { ApplyResult } from "../types";


export function WatchToggle() {
  const { scan, presetName, refreshHistory, rescan } = useSessionStore();
  const rules = useRuleStore((s) => s.rules);
  const [on, setOn] = useState(false);

  useEffect(() => {
    const applied = listen<ApplyResult>("zrename://watch-applied", (e) => {
      toast.success(
        `Renamed ${e.payload.renamed} new file${e.payload.renamed === 1 ? "" : "s"}`,
        "Picked up by the folder watch.",
      );
      refreshHistory();
      rescan(false);
    });
    const failed = listen<string>("zrename://watch-error", (e) => {
      toast.warn("New files were left alone", e.payload);
    });
    return () => {
      applied.then((f) => f()).catch(() => {});
      failed.then((f) => f()).catch(() => {});
    };

  }, []);


  useEffect(() => {
    if (on && (rules.length === 0 || !scan)) {
      api.watchStop().catch(() => {});
      setOn(false);
    }
  }, [on, rules.length, scan]);

  if (!scan) return null;

  async function toggle() {
    try {
      if (on) {
        await api.watchStop();
        setOn(false);
        toast.info("Stopped watching");
      } else {
        if (rules.length === 0) {
          toast.warn("Add a rule first", "A watch with no rules would do nothing.");
          return;
        }
        await api.watchStart(presetName);
        setOn(true);
        toast.info("Watching for new files", "New arrivals get the current rule stack.");
      }
    } catch (e) {
      toast.error("Could not change the watch", describeError(e));
    }
  }

  return (
    <button
      className="btn btn-ghost !py-1 !px-2 shrink-0"
      onClick={toggle}
      title={
        on
          ? "Stop applying the rules to new files"
          : "Apply the rules to files that arrive in this folder"
      }
      style={on ? { color: "var(--accent)", background: "var(--accent-soft)" } : undefined}
    >
      {on ? <Eye size={13} /> : <EyeOff size={13} />}
      <span className="text-[11px]">{on ? "watching" : "watch"}</span>
    </button>
  );
}
