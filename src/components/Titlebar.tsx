import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { Info, Maximize2, Minus, Settings, Square, X } from "lucide-react";

interface Props {
  onAbout: () => void;
  onSettings: () => void;
  subtitle?: string;
}

export function Titlebar({ onAbout, onSettings, subtitle }: Props) {
  const [maximized, setMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    win.isMaximized().then(setMaximized).catch(() => {});
    const unlisten = win.onResized(async () => {
      try {
        setMaximized(await win.isMaximized());
      } catch {

      }
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="flex items-center justify-between h-9 pl-3 pr-1 shrink-0 select-none"
      style={{ background: "var(--surface-2)", borderBottom: "1px solid var(--border)" }}
    >
      <div className="flex items-center gap-2 pointer-events-none min-w-0">
        <img src="/icon.png" alt="" className="w-[18px] h-[18px]" />
        <span className="text-[11px] font-bold tracking-[0.14em] uppercase" style={{ color: "var(--text-2)" }}>
          ZRename
        </span>
        {subtitle && (
          <span className="text-[11px] truncate" style={{ color: "var(--text-3)" }}>
            {subtitle}
          </span>
        )}
      </div>

      <div className="flex items-center gap-1" data-tauri-drag-region="false">
        <button className="btn btn-ghost !py-1 !px-2 text-[11px]" onClick={onSettings} title="Settings">
          <Settings size={12} />
          Settings
        </button>
        <button className="btn btn-ghost !py-1 !px-2 text-[11px]" onClick={onAbout} title="About">
          <Info size={12} />
          About
        </button>
        <div className="w-2" />
        <WinBtn label="Minimise" onClick={() => win.minimize()}>
          <Minus size={13} />
        </WinBtn>
        <WinBtn
          label={maximized ? "Restore" : "Maximise"}
          onClick={() => (maximized ? win.unmaximize() : win.maximize())}
        >
          {maximized ? <Square size={11} /> : <Maximize2 size={11} />}
        </WinBtn>
        <WinBtn label="Close" danger onClick={() => win.close()}>
          <X size={13} />
        </WinBtn>
      </div>
    </div>
  );
}

function WinBtn({
  children,
  onClick,
  label,
  danger,
}: {
  children: React.ReactNode;
  onClick: () => void;
  label: string;
  danger?: boolean;
}) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      className="w-9 h-7 flex items-center justify-center rounded transition-colors"
      style={{ color: "var(--text-3)" }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = danger ? "var(--collision)" : "var(--row-hover)";
        e.currentTarget.style.color = danger ? "#fff" : "var(--text)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
        e.currentTarget.style.color = "var(--text-3)";
      }}
    >
      {children}
    </button>
  );
}
