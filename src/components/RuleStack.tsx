import {
  DndContext, PointerSensor, closestCenter, useSensor, useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { RULE_HINTS, RULE_LABELS, RULE_ORDER } from "../lib/rules";
import { useRuleStore } from "../store/useRuleStore";
import { RuleCard } from "./RuleCard";
import { Popover } from "./ui";

export function RuleStack() {
  const { rules, selectedId, move, add } = useRuleStore();
  const [addOpen, setAddOpen] = useState(false);
  const addRef = useRef<HTMLButtonElement>(null);
  const closeAdd = useCallback(() => setAddOpen(false), []);


  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  useEffect(() => {
    const open = () => setAddOpen(true);
    window.addEventListener("zrename:add-rule", open);
    return () => window.removeEventListener("zrename:add-rule", open);
  }, []);

  function onDragEnd(e: DragEndEvent) {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    move(
      rules.findIndex((r) => r.id === active.id),
      rules.findIndex((r) => r.id === over.id),
    );
  }

  return (
    <div className="flex flex-col shrink-0">
      <div className="flex items-center justify-between px-3 py-2 shrink-0">
        <span className="label">Rules</span>
        <div className="relative">
          <button
            ref={addRef}
            className="btn btn-ghost !py-0.5 !px-1.5 text-[11px]"
            onClick={() => setAddOpen((v) => !v)}
          >
            <Plus size={12} />
            Add
          </button>
          <Popover anchor={addRef} open={addOpen} onDismiss={closeAdd} width={250}>
            <div>
              {RULE_ORDER.map((kind) => (
                <button
                  key={kind}
                  className="zr-menu-item"
                  onClick={() => {
                    add(kind);
                    setAddOpen(false);
                  }}
                >
                  <span className="min-w-0">
                    <span className="block">{RULE_LABELS[kind]}</span>
                    <span className="block text-[10.5px] leading-snug" style={{ color: "var(--text-3)" }}>
                      {RULE_HINTS[kind]}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          </Popover>
        </div>
      </div>

      <div className="px-2 pb-2">
        {rules.length === 0 ? (
          <button
            className="w-full rounded-[10px] py-6 px-3 text-[11.5px] leading-snug transition-colors"
            style={{ border: "1px dashed var(--border-strong)", color: "var(--text-3)" }}
            onClick={() => setAddOpen(true)}
          >
            No rules yet.
            <br />
            Add one, or load a preset.
          </button>
        ) : (
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <SortableContext items={rules.map((r) => r.id)} strategy={verticalListSortingStrategy}>
              <div className="flex flex-col gap-1.5">
                {rules.map((rule, i) => (
                  <RuleCard
                    key={rule.id}
                    rule={rule}
                    position={i + 1}
                    count={rules.length}
                    selected={rule.id === selectedId}
                  />
                ))}
              </div>
            </SortableContext>
          </DndContext>
        )}
      </div>
    </div>
  );
}
