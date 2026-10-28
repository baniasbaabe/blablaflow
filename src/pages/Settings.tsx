import { useEffect, useState } from "react";
import { Action, api, Gpu, Hotkeys, LANGUAGES, Theme } from "../api";
import { Button, Card, HotkeyInput, Page, Row, Select, Toggle, inputClass, useSettings } from "../ui";
import { GroqKey } from "./Groq";
import { ModelPicker } from "./Models";

const MAX_BINDINGS = 4;
const SHORTCUTS: [keyof Hotkeys, Action, string, string][] = [
  ["push_to_talk", "PushToTalk", "Push to talk", "Hold to dictate. Double-tap for hands-free."],
  ["hands_free", "HandsFree", "Hands-free", "Press to start, press again to stop."],
  ["command", "Command", "Command Mode", "Hold and say what to do with the selected text."],
  ["cancel", "Cancel", "Cancel", "Stops a dictation without pasting."],
  ["paste_last", "PasteLast", "Paste last transcript", ""],
  ["copy_last", "CopyLast", "Copy last transcript", ""],
  ["scratchpad", "Scratchpad", "Open Scratchpad", ""],
];
const DEFAULTS: Hotkeys = {
  push_to_talk: ["Ctrl+Win"], hands_free: ["Ctrl+Win+Space"], command: ["Ctrl+Win+Alt"], cancel: ["Escape"],
  paste_last: ["Shift+Alt+Z"], copy_last: ["Shift+Alt+X"], scratchpad: ["Win+Alt+S"],
};

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-8">
      <h2 className="mb-2 text-sm font-semibold">{title}</h2>
      <Card>{children}</Card>
    </section>
  );
}

export default function SettingsPage() {
  const [s, update] = useSettings();
  const [mics, setMics] = useState<string[]>([]);
  const [gpus, setGpus] = useState<Gpu[]>([]);
  const [groqModels, setGroqModels] = useState<string[]>([]);
  const [excluded, setExcluded] = useState("");

  useEffect(() => {
    api.listMics().then(setMics);
    api.listGpus().then(setGpus);
    api.listGroqModels().then(setGroqModels).catch(() => {});
  }, []);
  useEffect(() => { if (s) setExcluded(s.context_excluded_apps.join(", ")); }, [s === null]);
  if (!s) return null;

  const setBinding = (key: keyof Hotkeys, i: number, v: string | null) => {
    const list = [...s.hotkeys[key]];
    if (v) list[i] = v; else list.splice(i, 1);
    update({ hotkeys: { ...s.hotkeys, [key]: list } });
  };

  return (
    <Page title="Settings">
      <Section title="Shortcuts">
        {SHORTCUTS.map(([key, action, label, hint]) => (
          <Row key={key} label={label} hint={hint}>
            <div className="space-y-1">
              {s.hotkeys[key].map((b, i) => <HotkeyInput key={i} value={b} action={action} onChange={(v) => setBinding(key, i, v)} />)}
              {s.hotkeys[key].length < MAX_BINDINGS && (
                <HotkeyInput value={null} action={action} onChange={(v) => v && setBinding(key, s.hotkeys[key].length, v)} />
              )}
            </div>
          </Row>
        ))}
        <div className="pt-2 text-right"><Button variant="ghost" onClick={() => update({ hotkeys: DEFAULTS })}>Reset to defaults</Button></div>
      </Section>

      <Section title="Microphone & language">
        <Row label="Microphone" hint="If it's unplugged, the system default is used.">
          <Select value={s.mic ?? ""} onChange={(v) => update({ mic: v || null })}
            options={[["", "System default"], ...mics.map((m) => [m, m] as [string, string])]} />
        </Row>
        <Row label="Language" hint="Auto-detect handles switching between languages.">
          <Select value={s.language ?? ""} onChange={(v) => update({ language: v || null })}
            options={[["", "Auto-detect"], ...LANGUAGES]} />
        </Row>
      </Section>

      <Section title="Speech model (runs on this computer)">
        <ModelPicker active={s.model} onSelect={(model) => update({ model })} />
        <Toggle label="Use GPU" hint="Much faster. Turn off if dictation crashes or your GPU is busy." checked={s.use_gpu} onChange={(use_gpu) => update({ use_gpu })} />
        {s.use_gpu && (
          <Row label="GPU">
            {gpus.length ? (
              <Select value={String(s.gpu_device)} onChange={(v) => update({ gpu_device: Number(v) })}
                options={gpus.map((g) => [String(g.id), g.name] as [string, string])} />
            ) : <span className="text-sm text-neutral-500">No GPU found – using CPU</span>}
          </Row>
        )}
      </Section>

      <Section title="Cleanup with Groq">
        <GroqKey onModel={(groq_model) => { update({ groq_model }); api.listGroqModels().then(setGroqModels); }} />
        {groqModels.length > 0 && (
          <Row label="Model">
            <Select value={s.groq_model} onChange={(groq_model) => update({ groq_model })} options={groqModels.map((m) => [m, m] as [string, string])} />
          </Row>
        )}
      </Section>

      <Section title="Privacy">
        <Toggle label="Context awareness"
          hint="Sends up to 1,500 characters of text around your cursor to Groq so names and style match. Password fields are never read."
          checked={s.context_awareness} onChange={(context_awareness) => update({ context_awareness })} />
        {s.context_awareness && (
          <Row label="Never read context in" hint="App file names, comma-separated (e.g. keepass.exe)">
            <input className={inputClass} value={excluded} onChange={(e) => setExcluded(e.target.value)}
              onBlur={() => update({ context_excluded_apps: excluded.split(",").map((x) => x.trim().toLowerCase()).filter(Boolean) })} />
          </Row>
        )}
      </Section>

      <Section title="Flow bar & appearance">
        <Toggle label="Show the bar when idle" checked={s.bar_always_visible} onChange={(bar_always_visible) => update({ bar_always_visible })} />
        <Toggle label="Start and stop sounds" checked={s.sounds} onChange={(sounds) => update({ sounds })} />
        <Row label="Bar position" hint="Drag the bar to move it. Reset applies after restart.">
          <Button onClick={() => update({ bar_position: null })}>Reset position</Button>
        </Row>
        <Row label="Theme">
          <Select<Theme> value={s.theme} onChange={(theme) => { update({ theme }).then(() => window.dispatchEvent(new Event("settings-changed"))); }}
            options={[["System", "System"], ["Light", "Light"], ["Dark", "Dark"]]} />
        </Row>
      </Section>
    </Page>
  );
}
