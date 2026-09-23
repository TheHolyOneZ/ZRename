import { getCurrentWindow, type Window } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

type ResizeDirection = Parameters<Window["startResizeDragging"]>[0];

const HANDLES: { dir: ResizeDirection; cls: string }[] = [
  { dir: "North", cls: "n" },
  { dir: "South", cls: "s" },
  { dir: "East", cls: "e" },
  { dir: "West", cls: "w" },
  { dir: "NorthEast", cls: "ne" },
  { dir: "NorthWest", cls: "nw" },
  { dir: "SouthEast", cls: "se" },
  { dir: "SouthWest", cls: "sw" },
];

export function ResizeHandles() {
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

  if (maximized) return null;

  return (
    <>
      {HANDLES.map(({ dir, cls }) => (
        <div
          key={dir}
          className={`zr-resize zr-resize-${cls}`}
          onMouseDown={(e) => {
            if (e.button !== 0) return;
            e.preventDefault();
            win.startResizeDragging(dir).catch(() => {});
          }}
        />
      ))}
    </>
  );
}
