import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, HistoryEntry } from "../api";
import { Button, Page, inputClass } from "../ui";

function day(ms: number) {
  return new Date(ms).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
}

export default function Home() {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [error, setError] = useState("");

  const load = () => api.historyList(query).then(setItems).catch((e) => setError(String(e)));
  useEffect(() => { load(); }, [query]);
  useEffect(() => {
    const un = listen("history-changed", load);
    return () => { un.then((f) => f()); };
  }, [query]);

  const act = (p: Promise<unknown>) => p.then(load).catch((e) => setError(String(e)));
  const groups = new Map<string, HistoryEntry[]>();
  for (const e of items) groups.set(day(e.created_at), [...(groups.get(day(e.created_at)) ?? []), e]);

  return (
    <Page title="History" subtitle="Everything you dictated. Only stored on this computer.">
      <input className={`${inputClass} mb-6`} placeholder="Search…" value={query} onChange={(e) => setQuery(e.target.value)} />
      {error && <p className="mb-4 text-sm text-red-600">{error}</p>}
      {items.length === 0 && <p className="text-sm text-neutral-500">Nothing yet. Hold Ctrl+Win, speak, and let go.</p>}
      {[...groups].map(([d, entries]) => (
        <section key={d} className="mb-6">
          <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-neutral-500">{d}</h2>
          <div className="divide-y divide-neutral-200 rounded-xl border border-neutral-200 dark:divide-neutral-800 dark:border-neutral-800">
            {entries.map((e) => (
              <div key={e.id} className="group flex gap-4 p-3">
                <div className="w-12 shrink-0 pt-0.5 text-xs tabular-nums text-neutral-500">
                  {new Date(e.created_at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
                </div>
                <div className="min-w-0 flex-1">
                  {e.status === "Done"
                    ? <p className="whitespace-pre-wrap text-sm">{e.final_text}</p>
                    : <p className="text-sm text-neutral-500">{e.status === "Interrupted" ? "Interrupted – audio saved" : e.status}</p>}
                  <p className="mt-1 text-xs text-neutral-400">
                    {[e.kind !== "Dictation" && e.kind, e.app, e.words ? `${e.words} words` : ""].filter(Boolean).join(" · ")}
                  </p>
                </div>
                <div className="flex shrink-0 items-start gap-1 opacity-0 transition-opacity duration-150 ease-out group-hover:opacity-100 group-focus-within:opacity-100">
                  {e.status === "Done" && <Button variant="ghost" onClick={() => act(api.historyCopy(e.id, false))}>Copy</Button>}
                  {e.status === "Done" && e.raw_text !== e.final_text && e.kind === "Dictation" &&
                    <Button variant="ghost" onClick={() => act(api.historyCopy(e.id, true))}>Undo AI edit</Button>}
                  {(e.audio_path || e.raw_text) && <Button variant="ghost" onClick={() => act(api.historyRetry(e.id))}>Retry</Button>}
                  <Button variant="danger" onClick={() => act(api.historyDelete(e.id))}>Delete</Button>
                </div>
              </div>
            ))}
          </div>
        </section>
      ))}
    </Page>
  );
}
