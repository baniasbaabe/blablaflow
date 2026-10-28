import { useEffect, useState } from "react";
import { api, Transform } from "../api";
import { Button, Card, ErrorText, HotkeyInput, Page, Row, Select, inputClass, useSettings } from "../ui";

const MAX_CUSTOM = 8;
const MAX_SAMPLES = 5;
const blank: Transform = { id: 0, name: "", prompt: "", hotkey: null, samples: [], builtin: false };

export default function Transforms() {
  const [s, update] = useSettings();
  const [items, setItems] = useState<Transform[]>([]);
  const [edit, setEdit] = useState<Transform | null>(null);
  const [error, setError] = useState("");
  const load = () => api.transformsList().then(setItems);
  useEffect(() => { load(); }, []);
  if (!s) return null;

  const save = async () => {
    if (!edit) return;
    try {
      await api.transformSave({ ...edit, samples: edit.samples.filter((x) => x.trim()) });
      setEdit(null);
      setError("");
      load();
    } catch (e) {
      setError(String(e) === "limit reached" ? `You can have up to ${MAX_CUSTOM} custom transforms` : String(e));
    }
  };
  const custom = items.filter((t) => !t.builtin).length;

  return (
    <Page title="Transforms" subtitle="Select text anywhere, press a transform's shortcut, and it gets rewritten."
      actions={<Button variant="primary" disabled={custom >= MAX_CUSTOM} onClick={() => setEdit({ ...blank })}>New transform</Button>}>
      <Card className="mb-6">
        <Row label="Run after every dictation" hint="Applies a transform automatically to everything you dictate.">
          <Select value={String(s.auto_transform ?? "")} onChange={(v) => update({ auto_transform: v ? Number(v) : null })}
            options={[["", "Off"], ...items.map((t) => [String(t.id), t.name] as [string, string])]} />
        </Row>
      </Card>

      {edit && (
        <Card className="mb-6 space-y-3">
          <input className={inputClass} placeholder="Name" value={edit.name} disabled={edit.builtin} onChange={(e) => setEdit({ ...edit, name: e.target.value })} />
          <textarea className={`${inputClass} h-28`} placeholder="Instructions, e.g. Rewrite as a friendly tweet under 280 characters."
            value={edit.prompt} onChange={(e) => setEdit({ ...edit, prompt: e.target.value })} />
          <Row label="Shortcut"><HotkeyInput value={edit.hotkey} action={{ Transform: edit.id }} onChange={(hotkey) => setEdit({ ...edit, hotkey })} /></Row>
          <div>
            <div className="mb-1 text-sm font-medium">Writing samples (optional)</div>
            <p className="mb-2 text-xs text-neutral-500">Paste examples of your writing so the result sounds like you.</p>
            {edit.samples.map((x, i) => (
              <textarea key={i} className={`${inputClass} mb-2 h-20`} value={x}
                onChange={(e) => setEdit({ ...edit, samples: edit.samples.map((y, j) => (j === i ? e.target.value : y)) })} />
            ))}
            {edit.samples.length < MAX_SAMPLES && <Button onClick={() => setEdit({ ...edit, samples: [...edit.samples, ""] })}>Add sample</Button>}
          </div>
          <ErrorText>{error}</ErrorText>
          <div className="flex justify-end gap-2">
            <Button onClick={() => setEdit(null)}>Cancel</Button>
            <Button variant="primary" disabled={!edit.name.trim() || !edit.prompt.trim()} onClick={save}>Save</Button>
          </div>
        </Card>
      )}

      <div className="space-y-2">
        {items.map((t) => (
          <Card key={t.id} className="group flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="text-sm font-medium">{t.name}{t.builtin && <span className="ml-2 text-xs text-neutral-400">built-in</span>}</div>
              <p className="mt-1 line-clamp-2 text-sm text-neutral-500">{t.prompt}</p>
              <p className="mt-1 font-mono text-xs text-neutral-400">{t.hotkey ?? "No shortcut"}</p>
            </div>
            <div className="flex shrink-0 gap-1 opacity-0 transition-opacity duration-150 ease-out group-hover:opacity-100 group-focus-within:opacity-100">
              <Button variant="ghost" onClick={() => setEdit(t)}>Edit</Button>
              {!t.builtin && <Button variant="danger" onClick={() => api.transformDelete(t.id).then(load)}>Delete</Button>}
            </div>
          </Card>
        ))}
      </div>
    </Page>
  );
}
