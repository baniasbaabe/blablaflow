import { useEffect, useRef, useState } from "react";
import { api, Note } from "../api";

const MAX_TABS = 5;
const SAVE_DELAY_MS = 500;

function title(body: string) {
  return body.split("\n").find((l) => l.trim())?.slice(0, 40) || "Untitled";
}

export default function Scratchpad() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [tabs, setTabs] = useState<number[]>([]);
  const [current, setCurrent] = useState<Note | null>(null);
  const timer = useRef<number>(undefined);

  useEffect(() => {
    api.notesList().then((ns) => {
      setNotes(ns);
      if (ns[0]) { setTabs([ns[0].id]); setCurrent(ns[0]); } else newNote();
    });
  }, []);

  const newNote = async () => {
    const note = { id: 0, title: "Untitled", body: "", updated_at: 0 };
    const id = await api.noteSave(note);
    const n = { ...note, id };
    setNotes((ns) => [n, ...ns]);
    setTabs((t) => [...t, id].slice(-MAX_TABS));
    setCurrent(n);
  };

  const open = (n: Note) => {
    setTabs((t) => (t.includes(n.id) ? t : [...t, n.id].slice(-MAX_TABS)));
    setCurrent(n);
  };

  const close = (id: number) => {
    const rest = tabs.filter((t) => t !== id);
    setTabs(rest);
    if (current?.id === id) setCurrent(notes.find((n) => n.id === rest[rest.length - 1]) ?? null);
  };

  const change = (body: string) => {
    if (!current) return;
    const n = { ...current, body, title: title(body) };
    setCurrent(n);
    setNotes((ns) => ns.map((x) => (x.id === n.id ? n : x)));
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => api.noteSave(n), SAVE_DELAY_MS);
  };

  const remove = async (id: number) => {
    await api.noteDelete(id);
    setNotes((ns) => ns.filter((n) => n.id !== id));
    close(id);
  };

  return (
    <div className="flex h-full">
      <aside className="w-56 shrink-0 overflow-y-auto border-r border-neutral-200 p-3 dark:border-neutral-800">
        <button onClick={newNote} className="mb-2 w-full rounded-lg px-3 py-1.5 text-left text-sm font-medium hover:bg-neutral-100 dark:hover:bg-neutral-800">+ New note</button>
        {notes.map((n) => (
          <div key={n.id} className="group flex items-center">
            <button onClick={() => open(n)} className={`min-w-0 flex-1 truncate rounded-lg px-3 py-1.5 text-left text-sm ${current?.id === n.id ? "bg-neutral-100 dark:bg-neutral-800" : "text-neutral-600 dark:text-neutral-400"}`}>{n.title}</button>
            <button aria-label="Delete note" onClick={() => remove(n.id)} className="px-2 text-neutral-400 opacity-0 group-hover:opacity-100 hover:text-red-600">✕</button>
          </div>
        ))}
      </aside>
      <div className="flex flex-1 flex-col">
        <div className="flex gap-1 border-b border-neutral-200 px-3 pt-2 dark:border-neutral-800">
          {tabs.map((id) => {
            const n = notes.find((x) => x.id === id);
            return n && (
              <div key={id} className={`flex items-center rounded-t-lg px-3 py-1.5 text-sm ${current?.id === id ? "bg-neutral-100 font-medium dark:bg-neutral-800" : "text-neutral-500"}`}>
                <button onClick={() => setCurrent(n)} className="max-w-32 truncate">{n.title}</button>
                <button aria-label="Close tab" onClick={() => close(id)} className="ml-2 text-neutral-400 hover:text-neutral-900 dark:hover:text-white">✕</button>
              </div>
            );
          })}
        </div>
        {current ? (
          <textarea autoFocus className="flex-1 resize-none bg-transparent p-6 text-base leading-relaxed outline-none"
            placeholder="Click here and dictate…" value={current.body} onChange={(e) => change(e.target.value)} />
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-neutral-500">Open or create a note</div>
        )}
      </div>
    </div>
  );
}
