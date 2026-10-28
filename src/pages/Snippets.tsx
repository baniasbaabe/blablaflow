import { FormEvent, useEffect, useState } from "react";
import { api, Snippet } from "../api";
import { Button, Card, ErrorText, Page, inputClass } from "../ui";
import { ImportExport } from "./ListTools";

function message(e: unknown, cue: string) {
  const s = String(e);
  if (s === "already exists") return `A snippet for "${cue}" already exists`;
  if (s === "too long") return "Cue: 60 characters max · Text: 4,000 characters max";
  return s;
}

export default function Snippets() {
  const [items, setItems] = useState<Snippet[]>([]);
  const [edit, setEdit] = useState<Snippet>({ id: 0, cue: "", text: "" });
  const [error, setError] = useState("");
  const load = () => api.snippetsList().then(setItems);
  useEffect(() => { load(); }, []);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    try {
      if (edit.id) await api.snippetUpdate(edit);
      else await api.snippetAdd(edit.cue, edit.text);
      setEdit({ id: 0, cue: "", text: "" });
      setError("");
      load();
    } catch (err) {
      setError(message(err, edit.cue));
    }
  };

  return (
    <Page title="Snippets" subtitle="Say the cue, and blablaflow inserts the full text." actions={<ImportExport kind="snippets" onImported={load} />}>
      <Card className="mb-6">
        <form onSubmit={save} className="space-y-2">
          <input className={inputClass} placeholder='Cue, e.g. "my calendar link"' value={edit.cue} maxLength={60}
            onChange={(e) => setEdit({ ...edit, cue: e.target.value })} />
          <textarea className={`${inputClass} h-24`} placeholder="Text to insert" value={edit.text} maxLength={4000}
            onChange={(e) => setEdit({ ...edit, text: e.target.value })} />
          <div className="flex justify-end gap-2">
            {edit.id !== 0 && <Button onClick={() => setEdit({ id: 0, cue: "", text: "" })}>Cancel</Button>}
            <Button type="submit" variant="primary" disabled={!edit.cue.trim() || !edit.text.trim()}>{edit.id ? "Save" : "Add snippet"}</Button>
          </div>
        </form>
        <ErrorText>{error}</ErrorText>
      </Card>
      <div className="space-y-2">
        {items.map((s) => (
          <Card key={s.id} className="group flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="text-sm font-medium">"{s.cue}"</div>
              <p className="mt-1 line-clamp-3 whitespace-pre-wrap text-sm text-neutral-500">{s.text}</p>
            </div>
            <div className="flex shrink-0 gap-1 opacity-0 transition-opacity duration-150 ease-out group-hover:opacity-100 group-focus-within:opacity-100">
              <Button variant="ghost" onClick={() => setEdit(s)}>Edit</Button>
              <Button variant="danger" onClick={() => api.snippetDelete(s.id).then(load)}>Delete</Button>
            </div>
          </Card>
        ))}
      </div>
    </Page>
  );
}
