import { useEffect, useRef, useState } from "react";
import { Modal } from "./Modal";
import { TextField } from "./ui";

interface Props {
  open: boolean;
  title: string;
  label: string;
  hint?: string;
  initial?: string;
  confirmLabel?: string;
  placeholder?: string;
  onCancel: () => void;
  onConfirm: (value: string) => void;
}


export function PromptDialog({
  open, title, label, hint, initial = "", confirmLabel = "Save",
  placeholder, onCancel, onConfirm,
}: Props) {
  const [value, setValue] = useState(initial);
  const inputRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setValue(initial);

    const id = requestAnimationFrame(() => {
      inputRef.current?.querySelector("input")?.select();
    });
    return () => cancelAnimationFrame(id);
  }, [open, initial]);

  const trimmed = value.trim();

  function submit() {
    if (trimmed) onConfirm(trimmed);
  }

  return (
    <Modal open={open} onClose={onCancel} title={title} width={400}>
      <div
        className="flex flex-col gap-3"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            submit();
          }
        }}
      >
        <label className="flex flex-col gap-1.5">
          <span className="text-[11.5px]" style={{ color: "var(--text-2)" }}>{label}</span>
          <div ref={inputRef}>
            <TextField value={value} onChange={setValue} placeholder={placeholder} ariaLabel={label} />
          </div>
        </label>

        {hint && (
          <p className="text-[11px] leading-snug" style={{ color: "var(--text-3)" }}>{hint}</p>
        )}

        <div className="flex justify-end gap-2 pt-1">
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className="btn btn-accent" disabled={!trimmed} onClick={submit}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </Modal>
  );
}
