import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, XCircle } from "lucide-react";
import { RULE_LABELS } from "../lib/rules";
import { api } from "../lib/tauri";
import { useRuleStore } from "../store/useRuleStore";
import { usePreviewStore } from "../store/usePreviewStore";
import type { CaseStyle, RegexTest, RuleSpec, SortKey } from "../types";
import { Checkbox, Select as UiSelect, TextField, type Option } from "./ui";

export function RuleEditor({ rule }: { rule: RuleSpec }) {
  const update = useRuleStore((s) => s.update);
  const set = (patch: Partial<RuleSpec>) => update(rule.id, patch);

  return (
    <div className="flex flex-col gap-2.5 px-3 py-2.5">
      <div className="flex items-center justify-between">
        <span className="label">
          Rule · {RULE_LABELS[rule.kind]}
        </span>
        <ScopePicker rule={rule} set={set} />
      </div>

      <Body rule={rule} set={set} />
    </div>
  );
}

type Setter = (patch: Partial<RuleSpec>) => void;

function Body({ rule, set }: { rule: RuleSpec; set: Setter }) {
  switch (rule.kind) {
    case "replace":
      return <ReplaceForm rule={rule} set={set} />;

    case "case":
      return (
        <Row label="Style">
          <Select
            value={rule.style}
            onChange={(v) => set({ style: v as CaseStyle } as Partial<RuleSpec>)}
            options={[
              ["lower", "lowercase"], ["upper", "UPPERCASE"], ["title", "Title Case"],
              ["sentence", "Sentence case"], ["camel", "camelCase"], ["pascal", "PascalCase"],
              ["snake", "snake_case"], ["kebab", "kebab-case"],
            ]}
          />
        </Row>
      );

    case "insert":
      return (
        <>
          <Row label="Text">
            <Text value={rule.text} onChange={(v) => set({ text: v } as Partial<RuleSpec>)} placeholder="text or %token%" mono />
          </Row>
          <PositionPicker rule={rule} set={set} />
        </>
      );

    case "remove":
      return <RemoveForm rule={rule} set={set} />;

    case "trim":
      return (
        <div className="flex flex-wrap gap-x-4 gap-y-1.5">
          <Check label="Surrounding whitespace" checked={rule.whitespace} onChange={(v) => set({ whitespace: v } as Partial<RuleSpec>)} />
          <Check label="Collapse runs of spaces" checked={rule.collapse_spaces} onChange={(v) => set({ collapse_spaces: v } as Partial<RuleSpec>)} />
          <Row label="Also trim" inline>
            <Text value={rule.chars} onChange={(v) => set({ chars: v } as Partial<RuleSpec>)} placeholder="_-." mono width={90} />
          </Row>
        </div>
      );

    case "number":
      return (
        <>
          <div className="flex flex-wrap gap-x-4 gap-y-2 items-end">
            <Row label="Start" inline>
              <Num value={rule.start} onChange={(v) => set({ start: v } as Partial<RuleSpec>)} width={64} />
            </Row>
            <Row label="Step" inline>
              <Num value={rule.step} onChange={(v) => set({ step: v } as Partial<RuleSpec>)} width={64} />
            </Row>
            <Row label="Padding" inline>
              <Num value={rule.pad} min={1} max={12} onChange={(v) => set({ pad: v } as Partial<RuleSpec>)} width={64} />
            </Row>
            <Row label="Order by" inline>
              <Select
                value={rule.sort}
                onChange={(v) => set({ sort: v as SortKey } as Partial<RuleSpec>)}
                options={[
                  ["natural", "Name (natural)"], ["name", "Name (exact)"], ["size", "Size"],
                  ["modified", "Modified"], ["created", "Created"], ["scan", "Scan order"],
                ]}
              />
            </Row>
          </div>
          <div className="flex flex-wrap gap-x-4 gap-y-1.5">
            <Check label="Restart in each folder" checked={rule.reset_per_folder} onChange={(v) => set({ reset_per_folder: v } as Partial<RuleSpec>)} />
            <Check label="Descending" checked={rule.descending} onChange={(v) => set({ descending: v } as Partial<RuleSpec>)} />
          </div>
          <PositionPicker rule={rule} set={set} />
        </>
      );

    case "extension":
      return (
        <div className="flex flex-wrap gap-x-4 gap-y-2 items-end">
          <Row label="Do" inline>
            <Select
              value={rule.mode}
              onChange={(v) => set({ mode: v, ...(v === "set" || v === "fill" ? { ext: "ext" in rule ? rule.ext : "" } : {}) } as Partial<RuleSpec>)}
              options={[["lower", "lowercase"], ["upper", "UPPERCASE"], ["set", "Set to…"], ["fill", "Set when missing"], ["remove", "Remove"]]}
            />
          </Row>
          {(rule.mode === "set" || rule.mode === "fill") && (
            <Row label="Extension" inline>
              <Text value={rule.ext} onChange={(v) => set({ ext: v } as Partial<RuleSpec>)} placeholder="jpg" mono width={90} />
            </Row>
          )}
        </div>
      );

    case "sanitise":
      return (
        <>
          <div className="flex flex-wrap gap-x-4 gap-y-1.5">
            <Check label="Remove illegal characters" checked={rule.illegal} onChange={(v) => set({ illegal: v } as Partial<RuleSpec>)} />
            <Check label="Transliterate to ASCII" checked={rule.transliterate} onChange={(v) => set({ transliterate: v } as Partial<RuleSpec>)} />
            <Check label="Collapse spaces" checked={rule.collapse_spaces} onChange={(v) => set({ collapse_spaces: v } as Partial<RuleSpec>)} />
            <Check label="Trim trailing dots and spaces" checked={rule.trim_dots_spaces} onChange={(v) => set({ trim_dots_spaces: v } as Partial<RuleSpec>)} />
          </div>
          <Row label="Replace illegal characters with">
            <Text value={rule.replacement} onChange={(v) => set({ replacement: v } as Partial<RuleSpec>)} mono width={70} />
          </Row>
          <Hint>Judged against the filesystem the files are on, so a FAT32 stick is cleaned more strictly than an ext4 disk.</Hint>
        </>
      );

    case "template":
      return (
        <>
          <Row label="Name template">
            <Text value={rule.template} onChange={(v) => set({ template: v } as Partial<RuleSpec>)} placeholder="%exif:DateTimeOriginal:%Y-%m-%d%_%counter:3%" mono />
          </Row>
          <TokenHelp />
        </>
      );

    case "move_into":
      return (
        <>
          <Row label="Subfolder template">
            <Text value={rule.template} onChange={(v) => set({ template: v } as Partial<RuleSpec>)} placeholder="%exif:DateTimeOriginal:%Y%/%exif:DateTimeOriginal:%m%" mono />
          </Row>
          <Hint>Folders are created as needed. A leading <code>/</code> or <code>..</code> is ignored, so files stay under their current folder.</Hint>
        </>
      );

    case "csv_map":
      return (
        <>
          <Row label="Mapping file">
            <div className="flex gap-1.5 flex-1">
              <Text value={rule.path} onChange={(v) => set({ path: v } as Partial<RuleSpec>)} placeholder="old,new per line" mono />
              <button
                className="btn shrink-0"
                onClick={async () => {
                  const p = await open({ multiple: false, filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
                  if (typeof p === "string") set({ path: p } as Partial<RuleSpec>);
                }}
              >
                Browse
              </button>
            </div>
          </Row>
          <Check label="Match the whole filename, not just the name part" checked={rule.match_full_name} onChange={(v) => set({ match_full_name: v } as Partial<RuleSpec>)} />
          <Hint>Files missing from the list are left alone.</Hint>
        </>
      );
  }
}

function ReplaceForm({ rule, set }: { rule: Extract<RuleSpec, { kind: "replace" }>; set: Setter }) {
  const sample = usePreviewStore((s) => s.selectedRow);
  const [test, setTest] = useState<RegexTest | null>(null);

  const sampleName = sample?.fromName ?? "";

  useEffect(() => {
    if (!rule.regex || !rule.find) {
      setTest(null);
      return;
    }
    let live = true;
    api
      .regexTest(rule.find, sampleName, rule.with, rule.case_sensitive)
      .then((r) => live && setTest(r))
      .catch(() => live && setTest(null));
    return () => {
      live = false;
    };
  }, [rule.find, rule.with, rule.case_sensitive, rule.regex, sampleName]);

  return (
    <>
      <Row label="Find">
        <Text
          value={rule.find}
          onChange={(v) => set({ find: v } as Partial<RuleSpec>)}
          placeholder={rule.regex ? "^IMG_(\\d+)" : "text to find"}
          mono
          invalid={test?.valid === false}
        />
      </Row>
      <Row label="Replace with">
        <Text value={rule.with} onChange={(v) => set({ with: v } as Partial<RuleSpec>)} placeholder={rule.regex ? "shot-$1" : "replacement"} mono />
      </Row>
      <div className="flex flex-wrap gap-x-4 gap-y-1.5">
        <Check label="Regex" checked={rule.regex} onChange={(v) => set({ regex: v } as Partial<RuleSpec>)} />
        <Check label="Case sensitive" checked={rule.case_sensitive} onChange={(v) => set({ case_sensitive: v } as Partial<RuleSpec>)} />
        <Check label="Replace every match" checked={rule.all} onChange={(v) => set({ all: v } as Partial<RuleSpec>)} />
      </div>

      {rule.regex && rule.find !== "" && (
        <div
          className="rounded-lg px-2.5 py-2 text-[11.5px]"
          style={{ background: "var(--surface-2)", border: "1px solid var(--border)" }}
        >
          {test?.valid === false ? (
            <div className="flex items-start gap-1.5" style={{ color: "var(--collision)" }}>
              <XCircle size={13} className="shrink-0 mt-[1px]" />
              <span>{test.error}</span>
            </div>
          ) : !sampleName ? (
            <span style={{ color: "var(--text-3)" }}>Select a row in the preview to test against it.</span>
          ) : (
            <div className="flex flex-col gap-1">
              <div className="flex items-center gap-1.5">
                {test?.matched ? (
                  <CheckCircle2 size={13} style={{ color: "var(--ok)" }} className="shrink-0" />
                ) : (
                  <XCircle size={13} style={{ color: "var(--text-3)" }} className="shrink-0" />
                )}
                <span style={{ color: "var(--text-3)" }}>
                  {test?.matched ? "matches" : "no match on"}
                </span>
                <span className="mono truncate">{sampleName}</span>
              </div>

              {test?.matched && test.groups.length > 0 && (
                <div className="flex flex-wrap items-center gap-1.5 ml-[19px]">
                  <span style={{ color: "var(--text-3)" }}>groups:</span>
                  {test.groups.map((g, i) => (
                    <span key={i} className="chip mono">
                      ${i + 1} = {g === "" ? "(empty)" : g}
                    </span>
                  ))}
                </div>
              )}

              {test?.matched && test.preview != null && (
                <div className="flex items-center gap-1.5 ml-[19px]">
                  <span style={{ color: "var(--text-3)" }}>result:</span>
                  <span className="mono truncate" style={{ color: "var(--added)" }}>{test.preview}</span>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </>
  );
}

function RemoveForm({ rule, set }: { rule: Extract<RuleSpec, { kind: "remove" }>; set: Setter }) {
  return (
    <>
      <Row label="Remove">
        <Select
          value={rule.what}
          onChange={(v) => {
            const base = { what: v } as Record<string, unknown>;
            if (v === "range") Object.assign(base, { from: 0, to: 1 });
            if (v === "chars") Object.assign(base, { chars: "" });
            if (v === "word") Object.assign(base, { word: "", all: true });
            if (v === "duplicates") Object.assign(base, { text: "_" });
            set(base as Partial<RuleSpec>);
          }}
          options={[
            ["chars", "These characters"], ["word", "This text"], ["range", "A range of positions"],
            ["digits", "Every digit"], ["duplicates", "Repeated text"],
          ]}
        />
      </Row>

      {rule.what === "chars" && (
        <Row label="Characters">
          <Text value={rule.chars} onChange={(v) => set({ chars: v } as Partial<RuleSpec>)} placeholder="-_()" mono width={140} />
        </Row>
      )}
      {rule.what === "word" && (
        <>
          <Row label="Text">
            <Text value={rule.word} onChange={(v) => set({ word: v } as Partial<RuleSpec>)} placeholder=" copy" mono />
          </Row>
          <Check label="Remove every occurrence" checked={rule.all} onChange={(v) => set({ all: v } as Partial<RuleSpec>)} />
        </>
      )}
      {rule.what === "range" && (
        <div className="flex gap-4 items-end">
          <Row label="From" inline>
            <Num value={rule.from} onChange={(v) => set({ from: v } as Partial<RuleSpec>)} width={70} />
          </Row>
          <Row label="To" inline>
            <Num value={rule.to} onChange={(v) => set({ to: v } as Partial<RuleSpec>)} width={70} />
          </Row>
          <Hint>Counts characters. A negative number counts back from the end.</Hint>
        </div>
      )}
      {rule.what === "duplicates" && (
        <Row label="Collapse repeats of">
          <Text value={rule.text} onChange={(v) => set({ text: v } as Partial<RuleSpec>)} mono width={90} />
        </Row>
      )}
    </>
  );
}

function PositionPicker({ rule, set }: { rule: RuleSpec; set: Setter }) {
  if (!("at" in rule)) return null;
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-2 items-end">
      <Row label="Position" inline>
        <Select
          value={rule.at}
          onChange={(v) => {
            const base = { at: v } as Record<string, unknown>;
            if (v === "index") base.index = 0;
            if (v === "before" || v === "after") Object.assign(base, { marker: "", all: false });
            set(base as Partial<RuleSpec>);
          }}
          options={[["suffix", "At the end"], ["prefix", "At the start"], ["index", "At position…"], ["before", "Before a marker"], ["after", "After a marker"]]}
        />
      </Row>
      {rule.at === "index" && (
        <Row label="Index" inline>
          <Num value={rule.index} onChange={(v) => set({ index: v } as Partial<RuleSpec>)} width={70} />
        </Row>
      )}
      {(rule.at === "before" || rule.at === "after") && (
        <>
          <Row label="Marker" inline>
            <Text value={rule.marker} onChange={(v) => set({ marker: v } as Partial<RuleSpec>)} mono width={100} />
          </Row>
          <Check label="Every occurrence" checked={rule.all} onChange={(v) => set({ all: v } as Partial<RuleSpec>)} />
        </>
      )}
    </div>
  );
}

function ScopePicker({ rule, set }: { rule: RuleSpec; set: Setter }) {
  return (
    <div className="flex items-center gap-3">
      <span className="text-[10.5px]" style={{ color: "var(--text-3)" }}>apply to</span>
      <Check
        label="name"
        checked={rule.scope.stem}
        onChange={(v) => set({ scope: { ...rule.scope, stem: v } })}
      />
      <Check
        label="extension"
        checked={rule.scope.ext}
        onChange={(v) => set({ scope: { ...rule.scope, ext: v } })}
      />
    </div>
  );
}

function TokenHelp() {
  const tokens = [
    "%exif:DateTimeOriginal:%Y-%m-%d%", "%exif:Model%", "%id3:artist%", "%id3:track%",
    "%video:height%", "%pdf:title%", "%file:created%", "%folder:name%", "%hash:crc32%", "%counter:3%",
  ];
  return (
    <div className="flex flex-wrap gap-1">
      {tokens.map((t) => (
        <span key={t} className="chip mono !text-[10px]">{t}</span>
      ))}
    </div>
  );
}

function Row({
  label, children, inline,
}: { label: string; children: React.ReactNode; inline?: boolean }) {
  if (inline) {
    return (
      <label className="flex flex-col gap-1">
        <span className="text-[10.5px]" style={{ color: "var(--text-3)" }}>{label}</span>
        {children}
      </label>
    );
  }
  return (
    <label className="flex items-center gap-2">
      <span className="text-[11px] w-[150px] shrink-0" style={{ color: "var(--text-2)" }}>{label}</span>
      <div className="flex-1 min-w-0 flex">{children}</div>
    </label>
  );
}

function Text({
  value, onChange, placeholder, mono, width, invalid,
}: {
  value: string; onChange: (v: string) => void; placeholder?: string;
  mono?: boolean; width?: number; invalid?: boolean;
}) {
  return (
    <TextField
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      mono={mono}
      width={width}
      invalid={invalid}
    />
  );
}

function Num({
  value, onChange, width, min, max,
}: { value: number; onChange: (v: number) => void; width?: number; min?: number; max?: number }) {
  return (
    <TextField
      type="number"
      value={Number.isFinite(value) ? value : 0}
      min={min}
      max={max}
      width={width}
      onChange={(v) => {
        const n = Number(v);
        onChange(Number.isFinite(n) ? n : 0);
      }}
    />
  );
}

function Select({
  value, onChange, options,
}: { value: string; onChange: (v: string) => void; options: [string, string][] }) {
  const opts: Option[] = options.map(([value, label]) => ({ value, label }));
  return <UiSelect value={value} onChange={onChange} options={opts} />;
}

function Check({
  label, checked, onChange,
}: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return <Checkbox checked={checked} onChange={onChange} label={label} />;
}

function Hint({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-[11px] leading-snug" style={{ color: "var(--text-3)" }}>
      {children}
    </p>
  );
}
