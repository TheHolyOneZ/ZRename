import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useToastStore, type ToastKind } from "../store/useToastStore";

const ICONS: Record<ToastKind, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  warn: AlertTriangle,
  error: XCircle,
};

const COLORS: Record<ToastKind, string> = {
  info: "var(--text-2)",
  success: "var(--ok)",
  warn: "var(--warn)",
  error: "var(--collision)",
};

export function Toasts() {
  const { toasts, dismiss } = useToastStore();
  if (toasts.length === 0) return null;

  return (
    <div className="fixed right-4 z-50 flex flex-col gap-2 max-w-[380px]"
      style={{ bottom: "var(--toast-bottom, 1rem)" }}>
      {toasts.map((t) => {
        const Icon = ICONS[t.kind];
        return (
          <div
            key={t.id}
            role="status"
            className="panel fade-in flex items-start gap-2.5 px-3 py-2.5"
            style={{ background: "var(--raised)", boxShadow: "var(--shadow)" }}
          >
            <Icon size={15} style={{ color: COLORS[t.kind], flexShrink: 0, marginTop: 1 }} />
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px] leading-snug">{t.message}</div>
              {t.detail && (
                <div className="text-[11.5px] mt-0.5 leading-snug" style={{ color: "var(--text-3)" }}>
                  {t.detail}
                </div>
              )}
            </div>
            <button
              className="btn btn-ghost !p-1 shrink-0"
              onClick={() => dismiss(t.id)}
              aria-label="Dismiss"
            >
              <X size={12} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
