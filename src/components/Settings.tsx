import { useSessionStore } from "../store/useSessionStore";
import { useSettingsStore } from "../store/useSettingsStore";
import { THEMES } from "../types";
import { Modal } from "./Modal";
import { Select, TextField, Toggle } from "./ui";

export function Settings({ open, onClose }: { open: boolean; onClose: () => void }) {
  const s = useSettingsStore();
  const { caps, setScanOptions, scanOptions, setLongPaths, setMissingToken } = useSessionStore();
  const longPaths = s.longPaths;
  const setLongPathsSetting = s.set;

  return (
    <Modal open={open} onClose={onClose} title="Settings" width={480}>
      <div className="flex flex-col gap-5">
        <Section label="Theme">
          <div className="grid grid-cols-2 gap-1.5">
            {THEMES.map((t) => (
              <button
                key={t.id}
                className="btn justify-center"
                style={
                  s.theme === t.id
                    ? { borderColor: "var(--accent)", background: "var(--accent-soft)" }
                    : undefined
                }
                onClick={() => s.setTheme(t.id)}
              >
                {t.label}
              </button>
            ))}
          </div>
        </Section>

        <Section label="Preview">
          <Check
            label="Comfortable row height"
            hint="34 px rows instead of 26."
            checked={s.comfortable}
            onChange={(v) => s.set("comfortable", v)}
          />
          <Check
            label="Sort collisions to the top"
            checked={s.collisionsFirst}
            onChange={(v) => s.set("collisionsFirst", v)}
          />
        </Section>

        <Section label="Safety">
          <Check
            label="Paranoid mode"
            hint="Hardlink every original into a temporary directory before renaming. Costs nothing on the same filesystem."
            checked={s.paranoid}
            onChange={(v) => s.set("paranoid", v)}
          />
          <Check
            label="Allow paths past the platform limit"
            hint="Windows caps paths at 260 characters for most software. ZRename itself reaches past it, so this is on by default; turn it off to be warned about names other tools would not open."
            checked={longPaths}
            onChange={(v) => {
              setLongPathsSetting("longPaths", v);
              setLongPaths(v);
            }}
          />
          <Check
            label="Follow symlinks when scanning"
            hint="Off by default: a link loop would otherwise be walked forever."
            checked={scanOptions.follow_symlinks}
            onChange={(v) => setScanOptions({ follow_symlinks: v })}
          />
        </Section>

        <Section label="Tokens">
          <label className="flex flex-col gap-1.5">
            <span className="text-[11.5px]" style={{ color: "var(--text-2)" }}>
              When a file has no value for a token
            </span>
            <Select
              value={s.missingToken}
              ariaLabel="When a file has no value for a token"
              onChange={(v) => {
                s.set("missingToken", v as never);
                setMissingToken(v as never);
              }}
              options={[
                { value: "placeholder", label: "Use the stand-in", hint: "Rename it anyway" },
                { value: "skip", label: "Leave the file alone", hint: "A photo with no date keeps its name" },
              ]}
            />
          </label>

          <label className="flex items-center gap-2">
            <span className="text-[11.5px] flex-1" style={{ color: "var(--text-2)" }}>
              Stand-in when a token has no value
            </span>
            <TextField
              mono
              width={90}
              ariaLabel="Stand-in for an unresolved token"
              value={s.placeholder}
              onChange={(v) => s.set("placeholder", v)}
            />
          </label>
          <p className="text-[11px] leading-snug" style={{ color: "var(--text-3)" }}>
            Video tokens need <code className="mono">ffprobe</code>:{" "}
            {caps?.ffprobe ? (
              <span style={{ color: "var(--ok)" }}>found on this machine.</span>
            ) : (
              <span style={{ color: "var(--warn)" }}>
                not found, so <code className="mono">%video:…%</code> resolves to the stand-in.
              </span>
            )}
          </p>
        </Section>

        {caps && (
          <Section label="Files">
            <Path label="Presets" value={caps.presetDir} />
            <Path label="Undo journal" value={caps.journalDir} />
          </Section>
        )}
      </div>
    </Modal>
  );
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="label">{label}</span>
      {children}
    </div>
  );
}

function Check({
  label, hint, checked, onChange,
}: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return <Toggle checked={checked} onChange={onChange} label={label} hint={hint} />;
}

function Path({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[11px]" style={{ color: "var(--text-2)" }}>{label}</span>
      <code className="mono text-[10.5px] break-all" style={{ color: "var(--text-3)" }}>{value}</code>
    </div>
  );
}
