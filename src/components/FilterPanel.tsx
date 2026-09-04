import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { parseDate, parseSize } from "../lib/format";
import { useSessionStore } from "../store/useSessionStore";
import { Checkbox, Select, TextField } from "./ui";

export function FilterPanel() {
  const { scanOptions, setScanOptions, conflict, setConflict } = useSessionStore();
  const [open, setOpen] = useState(true);
  const [more, setMore] = useState(false);


  const [minText, setMinText] = useState("");
  const [maxText, setMaxText] = useState("");
  const [afterText, setAfterText] = useState("");
  const [beforeText, setBeforeText] = useState("");

  const list = (v: string[]) => v.join(", ");
  const parse = (v: string) => v.split(",").map((s) => s.trim()).filter(Boolean);

  const minSize = parseSize(minText);
  const maxSize = parseSize(maxText);
  const after = parseDate(afterText);
  const before = parseDate(beforeText, true);

  const active =
    scanOptions.extensions.length +
    scanOptions.exclude_globs.length +
    scanOptions.include_globs.length +
    (scanOptions.name_regex ? 1 : 0) +
    (scanOptions.min_size !== null ? 1 : 0) +
    (scanOptions.max_size !== null ? 1 : 0) +
    (scanOptions.modified_after !== null ? 1 : 0) +
    (scanOptions.modified_before !== null ? 1 : 0);

  return (
    <div className="shrink-0" style={{ borderTop: "1px solid var(--border)" }}>
      <button className="w-full flex items-center gap-1 px-3 py-2" onClick={() => setOpen((v) => !v)}>
        {open ? <ChevronDown size={12} style={{ color: "var(--text-3)" }} /> : <ChevronRight size={12} style={{ color: "var(--text-3)" }} />}
        <span className="label">Filters</span>
        {active > 0 && (
          <span className="chip !py-0" style={{ color: "var(--accent)", borderColor: "var(--accent)" }}>
            {active}
          </span>
        )}
      </button>

      {open && (
        <div className="px-3 pb-3 flex flex-col gap-2">
          <Field label="Extensions">
            <TextField
              mono
              placeholder="jpg, png, pdf"
              value={list(scanOptions.extensions)}
              onChange={(v) => setScanOptions({ extensions: parse(v) })}
            />
          </Field>

          <Field label="Exclude">
            <TextField
              mono
              placeholder="*.tmp, .git/*"
              value={list(scanOptions.exclude_globs)}
              onChange={(v) => setScanOptions({ exclude_globs: parse(v) })}
            />
          </Field>

          <Check
            label="Include subfolders"
            checked={scanOptions.recursive}
            onChange={(v) => setScanOptions({ recursive: v, max_depth: v ? scanOptions.max_depth : null })}
          />

          {scanOptions.recursive && (
            <Field label="Depth limit">
              <TextField
                type="number"
                min={0}
                width={80}
                placeholder="all"
                value={scanOptions.max_depth ?? ""}
                onChange={(v) => setScanOptions({ max_depth: v === "" ? null : Number(v) })}
              />
            </Field>
          )}

          <Check label="Rename folders too" checked={scanOptions.include_dirs} onChange={(v) => setScanOptions({ include_dirs: v })} />
          <Check label="Include hidden files" checked={scanOptions.include_hidden} onChange={(v) => setScanOptions({ include_hidden: v })} />

          <button
            className="flex items-center gap-1 text-[11px] mt-0.5 self-start"
            style={{ color: "var(--text-3)" }}
            onClick={() => setMore((v) => !v)}
          >
            {more ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
            {more ? "Fewer filters" : "More filters"}
          </button>

          {more && (
            <div className="flex flex-col gap-2 pl-1" style={{ borderLeft: "1px solid var(--border)" }}>
              <div className="pl-2 flex flex-col gap-2">
                <Field label="Only these (glob)">
                  <TextField
                    mono
                    placeholder="IMG_*, DSC_*"
                    value={list(scanOptions.include_globs)}
                    onChange={(v) => setScanOptions({ include_globs: parse(v) })}
                  />
                </Field>

                <Field label="Name matches (regex)">
                  <TextField
                    mono
                    placeholder="^S\\d{2}E\\d{2}"
                    value={scanOptions.name_regex ?? ""}
                    onChange={(v) => setScanOptions({ name_regex: v === "" ? null : v })}
                  />
                </Field>

                <div className="flex gap-2">
                  <Field label="Larger than">
                    <TextField
                      mono
                      placeholder="1MB"
                      invalid={minSize === undefined}
                      value={minText}
                      onChange={(v) => {
                        setMinText(v);
                        const n = parseSize(v);
                        if (n !== undefined) setScanOptions({ min_size: n });
                      }}
                    />
                  </Field>
                  <Field label="Smaller than">
                    <TextField
                      mono
                      placeholder="500MB"
                      invalid={maxSize === undefined}
                      value={maxText}
                      onChange={(v) => {
                        setMaxText(v);
                        const n = parseSize(v);
                        if (n !== undefined) setScanOptions({ max_size: n });
                      }}
                    />
                  </Field>
                </div>

                <div className="flex gap-2">
                  <Field label="Modified after">
                    <TextField
                      type="date"
                      invalid={after === undefined}
                      value={afterText}
                      onChange={(v) => {
                        setAfterText(v);
                        const n = parseDate(v);
                        if (n !== undefined) setScanOptions({ modified_after: n });
                      }}
                    />
                  </Field>
                  <Field label="Modified before">
                    <TextField
                      type="date"
                      invalid={before === undefined}
                      value={beforeText}
                      onChange={(v) => {
                        setBeforeText(v);
                        const n = parseDate(v, true);
                        if (n !== undefined) setScanOptions({ modified_before: n });
                      }}
                    />
                  </Field>
                </div>
              </div>
            </div>
          )}

          <Field label="When a name is taken">
            <Select
              value={conflict}
              onChange={(v) => setConflict(v as never)}
              ariaLabel="What to do when a name is taken"
              options={[
                { value: "stop", label: "Stop and report it", hint: "Nothing is renamed" },
                { value: "skip", label: "Skip that file", hint: "Rename the rest" },
                { value: "suffix", label: "Add (2), (3)…", hint: "Keep both" },
                { value: "overwrite", label: "Overwrite it", hint: "Destroys what is there" },
              ]}
            />
          </Field>

          {conflict === "overwrite" && (
            <p className="text-[10.5px] leading-snug" style={{ color: "var(--collision)" }}>
              Overwriting destroys the file already at that name. It never applies
              to two selected files wanting the same name — one of them would
              simply be lost.
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1 min-w-0 flex-1">
      <span className="text-[10.5px]" style={{ color: "var(--text-3)" }}>{label}</span>
      {children}
    </label>
  );
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return <Checkbox checked={checked} onChange={onChange} label={label} />;
}
