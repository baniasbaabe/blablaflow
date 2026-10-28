import { FormEvent, useEffect, useState } from "react";
import { api, DictEntry } from "../api";
import { Button, Card, ErrorText, Page, inputClass } from "../ui";
import { ImportExport } from "./ListTools";

export default function Dictionary() {
  const [items, setItems] = useState<DictEntry[]>([]);
  const [term, setTerm] = useState("");
  const [replacement, setReplacement] = useState("");
  const [error, setError] = useState("");
  const load = () => api.dictList().then(setItems);
  useEffect(() => { load(); }, []);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    try {
      await api.dictAdd(term, replacement || null);
      setTerm("");
      setReplacement("");
      setError("");
      load();
    } catch (err) {
      setError(String(err) === "already exists" ? `"${term}" is already in your dictionary` : String(err) === "too long" ? "Use 60 characters or fewer" : String(err));
    }
  };

  return (
    <Page title="Dictionary" subtitle="Names, jargon and spellings blablaflow should get right." actions={<ImportExport kind="dictionary" onImported={load} />}>
      <Card className="mb-6">
        <form onSubmit={add} className="flex gap-2">
          <input className={inputClass} placeholder="Word or name, e.g. Kubernetes" value={term} maxLength={60} onChange={(e) => setTerm(e.target.value)} />
          <input className={inputClass} placeholder="Replace with (optional)" value={replacement} maxLength={60} onChange={(e) => setReplacement(e.target.value)} />
          <Button type="submit" variant="primary" disabled={!term.trim()}>Add</Button>
        </form>
        <ErrorText>{error}</ErrorText>
        <p className="mt-2 text-xs text-neutral-500">With a replacement, blablaflow swaps the word whenever it appears (e.g. "wisper" → "Wispr").</p>
      </Card>
      <div className="flex flex-wrap gap-2">
        {items.map((d) => (
          <span key={d.id} className="group flex items-center gap-1 rounded-full border border-neutral-200 py-1 pr-1 pl-3 text-sm dark:border-neutral-800">
            {d.term}{d.replacement && <span className="text-neutral-500"> → {d.replacement}</span>}
            <button aria-label={`Remove ${d.term}`} onClick={() => api.dictDelete(d.id).then(load)}
              className="rounded-full px-1.5 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-900 dark:hover:bg-neutral-800">✕</button>
          </span>
        ))}
      </div>
    </Page>
  );
}
