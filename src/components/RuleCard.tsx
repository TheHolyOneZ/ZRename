import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ArrowDown, ArrowUp, Copy, GripVertical, MoreVertical, Pencil, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  MAX_LABEL, RULE_LABELS, cleanLabel, isConfigured, ruleTitle, scopeLabel, summariseRule,
} from "../lib/rules";
import { useRuleStore } from "../store/useRuleStore";
import type { RuleSpec } from "../types";
import { Checkbox, Popover } from "./ui";

interface Props {
  rule: RuleSpec;
  position: number;
  count: number;
  selected: boolean;
}

export function RuleCard({ rule, position, count, selected }: Props) {
  const {
    attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging,
  } = useSortable({ id: rule.id });
  const { select, toggle, remove, duplicate, nudge, update } = useRuleStore();
  const [menuOpen, setMenuOpen] = useState(false);
  const kebabRef = useRef<HTMLButtonElement>(null);
  const [renaming, setRenaming] = useState(false);

  const closeMenu = useCallback(() => setMenuOpen(false), []);

  useEffect(() => {
    const start = (e: Event) => {
      if ((e as CustomEvent<string>).detail === rule.id) setRenaming(true);
    };
    window.addEventListener("zrename:rename-rule", start);
    return () => window.removeEventListener("zrename:rename-rule", start);
  }, [rule.id]);

  const configured = isConfigured(rule);
  const title = ruleTitle(rule);
  const typeLabel = RULE_LABELS[rule.kind];
  const named = !!rule.label?.trim();

  return (
    <div
      ref={setNodeRef}
      className="rule-card px-2 py-1.5 group"
      data-enabled={rule.enabled}
      data-selected={selected}
      data-dragging={isDragging}
      style={{
        transform: CSS.Transform.toString(transform),
        transition: transition ?? "transform 160ms cubic-bezier(0.2, 0, 0, 1)",
        zIndex: isDragging ? 10 : undefined,
        position: "relative",

        touchAction: "none",
      }}
      onClick={() => select(rule.id)}
    >
      <div className="flex items-center gap-1.5">
        <button
          ref={setActivatorNodeRef}
          className="grip shrink-0 py-1"
          {...attributes}
          {...listeners}
          aria-label="Reorder"
          title="Drag to reorder, or use Alt+Up / Alt+Down"
          onClick={(e) => e.stopPropagation()}
        >
          <GripVertical size={13} />
        </button>

        <span className="shrink-0 flex">
          <Checkbox
            checked={rule.enabled}
            onChange={() => toggle(rule.id)}
            stopPropagation
            ariaLabel={`${rule.enabled ? "Disable" : "Enable"} ${title}`}
          />
        </span>

        <span className="text-[10.5px] tabular-nums shrink-0" style={{ color: "var(--text-3)" }}>
          {position}
        </span>

        {renaming ? (
          <RenameField
            initial={rule.label ?? ""}
            placeholder={typeLabel}
            onDone={(text) => {
              setRenaming(false);
              if (text !== null) update(rule.id, { label: cleanLabel(text) });
            }}
          />
        ) : (
          <span
            className="flex items-baseline gap-1.5 min-w-0 flex-1"
            title={named ? `${title} (${typeLabel}) · double-click or F2 to rename` : "Double-click or F2 to rename"}
            onDoubleClick={(e) => {
              e.stopPropagation();
              setRenaming(true);
            }}
          >
            <span
              className="text-[12px] font-medium truncate"
              style={{ color: rule.enabled ? "var(--text)" : "var(--text-3)" }}
            >
              {title}
            </span>
            {named && (
              <span className="text-[10px] shrink-0" style={{ color: "var(--text-3)" }}>
                {typeLabel}
              </span>
            )}
          </span>
        )}

        {!configured && (
          <span className="chip shrink-0" style={{ color: "var(--warn)", borderColor: "var(--warn)" }}>
            set up
          </span>
        )}

        <div className="relative shrink-0">
          <button
            ref={kebabRef}
            className="btn btn-ghost !p-1 opacity-0 group-hover:opacity-100 focus:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              setMenuOpen((v) => !v);
            }}
            aria-label="Rule actions"
          >
            <MoreVertical size={13} />
          </button>

          <Popover anchor={kebabRef} open={menuOpen} onDismiss={closeMenu} width={150}>
            <div onClick={(e) => e.stopPropagation()}>
              <MenuItem
                icon={<Pencil size={12} />}
                label="Rename"
                onClick={() => {
                  setMenuOpen(false);
                  setRenaming(true);
                }}
              />
              <MenuItem
                icon={<ArrowUp size={12} />}
                label="Move up"
                disabled={position === 1}
                onClick={() => {
                  nudge(rule.id, -1);
                  setMenuOpen(false);
                }}
              />
              <MenuItem
                icon={<ArrowDown size={12} />}
                label="Move down"
                disabled={position === count}
                onClick={() => {
                  nudge(rule.id, 1);
                  setMenuOpen(false);
                }}
              />
              <MenuItem
                icon={<Copy size={12} />}
                label="Duplicate"
                onClick={() => {
                  duplicate(rule.id);
                  setMenuOpen(false);
                }}
              />
              <MenuItem
                icon={<Trash2 size={12} />}
                label="Delete"
                danger
                onClick={() => {
                  remove(rule.id);
                  setMenuOpen(false);
                }}
              />
            </div>
          </Popover>
        </div>
      </div>

      <div
        className="text-[11px] truncate mt-0.5 ml-[26px] mr-1"
        style={{ color: rule.enabled ? "var(--text-3)" : "var(--border-strong)" }}
        title={summariseRule(rule)}
      >
        {summariseRule(rule)}
        {rule.scope.ext && (
          <span className="ml-1.5" style={{ color: "var(--text-3)" }}>
            · {scopeLabel(rule.scope)}
          </span>
        )}
      </div>
    </div>
  );
}

function RenameField({
  initial,
  placeholder,
  onDone,
}: {
  initial: string;
  placeholder: string;

  onDone: (text: string | null) => void;
}) {
  const [text, setText] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);

  useEffect(() => {
    const id = requestAnimationFrame(() => {
      ref.current?.focus();
      ref.current?.select();
    });
    return () => cancelAnimationFrame(id);
  }, []);

  function finish(value: string | null) {
    if (done.current) return;
    done.current = true;
    onDone(value);
  }

  return (
    <input
      ref={ref}
      className="zr-input text-[12px] font-medium flex-1 min-w-0 !py-0 !px-1.5"
      value={text}
      placeholder={placeholder}
      maxLength={MAX_LABEL}
      aria-label="Rule name"
      spellCheck={false}
      autoComplete="off"
      onChange={(e) => setText(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onPointerDown={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          finish(text);
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          finish(null);
        }
      }}
      onBlur={() => finish(text)}
    />
  );
}

function MenuItem({
  icon,
  label,
  onClick,
  danger,
  disabled,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      className="zr-menu-item items-center"
      disabled={disabled}
      style={{
        color: danger ? "var(--collision)" : "var(--text)",
        opacity: disabled ? 0.4 : 1,
      }}
      onClick={(e) => {
        e.stopPropagation();
        if (!disabled) onClick();
      }}
    >
      {icon}
      {label}
    </button>
  );
}
