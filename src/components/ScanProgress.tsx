import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { formatCount } from "../lib/format";
import { useSessionStore } from "../store/useSessionStore";

export function ScanProgress() {
  const scanning = useSessionStore((s) => s.scanning);
  const scanned = useSessionStore((s) => s.scanned);
  const [shown, setShown] = useState(false);
  const [started, setStarted] = useState(0);
  const [now, setNow] = useState(0);

  useEffect(() => {
    if (!scanning) {
      setShown(false);
      return;
    }
    const t0 = Date.now();
    setStarted(t0);
    setNow(t0);
    const show = setTimeout(() => setShown(true), 300);
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => {
      clearTimeout(show);
      clearInterval(tick);
    };
  }, [scanning]);

  if (!scanning || !shown) return null;

  const secs = Math.max(0, Math.floor((now - started) / 1000));

  return (
    <div
      className="absolute inset-0 z-30 flex items-center justify-center fade-in"
      style={{ background: "rgba(0,0,0,0.25)" }}
      role="status"
      aria-live="polite"
    >
      <div
        className="panel flex items-center gap-3 px-4 py-3"
        style={{ background: "var(--raised)", boxShadow: "var(--shadow)" }}
      >
        <Loader2 size={18} className="animate-spin shrink-0" style={{ color: "var(--accent)" }} />
        <div className="flex flex-col">
          <span className="text-[12.5px] font-semibold">Reading the folder…</span>
          <span className="text-[11.5px] tabular-nums" style={{ color: "var(--text-3)" }}>
            {scanned > 0 ? `${formatCount(scanned)} items checked` : "Waiting for the first files"}
            {secs >= 2 && ` · ${secs}s`}
          </span>
        </div>
      </div>
    </div>
  );
}
