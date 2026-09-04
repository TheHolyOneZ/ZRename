import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  width?: number;
}

export function Modal({ open, onClose, title, children, width = 460 }: Props) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center p-6"
      style={{ background: "rgba(0,0,0,0.45)" }}
      onMouseDown={onClose}
    >
      <div
        className="panel fade-in max-h-full overflow-y-auto"
        style={{ background: "var(--raised)", boxShadow: "var(--shadow)", width, maxWidth: "92vw" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div
          className="flex items-center justify-between px-4 py-2.5 sticky top-0"
          style={{ borderBottom: "1px solid var(--border)", background: "var(--raised)" }}
        >
          <span className="text-[13px] font-semibold">{title}</span>
          <button className="btn btn-ghost !p-1" onClick={onClose} aria-label="Close">
            <X size={13} />
          </button>
        </div>
        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}
