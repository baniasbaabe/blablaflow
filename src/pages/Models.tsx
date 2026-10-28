import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, LocalModel } from "../api";
import { Button } from "../ui";

/** Model list with download progress; selecting a model saves it as the active one. */
export function ModelPicker({ active, onSelect }: { active: string; onSelect: (file: string) => void }) {
  const [models, setModels] = useState<LocalModel[]>([]);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [error, setError] = useState("");
  const load = () => api.listModels().then(setModels);

  useEffect(() => {
    load();
    const un = listen<{ file: string; done: number; total: number }>("model-progress", (e) =>
      setProgress((p) => ({ ...p, [e.payload.file]: e.payload.total ? e.payload.done / e.payload.total : 0 })));
    return () => { un.then((f) => f()); };
  }, []);

  const download = async (file: string) => {
    setError("");
    setProgress((p) => ({ ...p, [file]: 0 }));
    try {
      await api.downloadModel(file);
      onSelect(file);
    } catch (e) {
      setError(String(e));
    }
    setProgress(({ [file]: _, ...rest }) => rest);
    load();
  };

  return (
    <div className="space-y-2">
      {models.map((m) => (
        <div key={m.file} className="flex items-center justify-between gap-4 rounded-lg border border-neutral-200 p-3 dark:border-neutral-800">
          <div>
            <div className="text-sm font-medium">{m.label}{active === m.file && m.downloaded && <span className="ml-2 text-xs text-emerald-600">in use</span>}</div>
            <div className="text-xs text-neutral-500">{m.size_mb} MB</div>
            {m.file in progress && (
              <div className="mt-2 h-1.5 w-48 overflow-hidden rounded-full bg-neutral-200 dark:bg-neutral-800">
                <div className="h-full origin-left bg-neutral-900 transition-transform duration-200 ease-linear dark:bg-white"
                  style={{ transform: `scaleX(${progress[m.file]})` }} />
              </div>
            )}
          </div>
          <div className="flex gap-1">
            {!m.downloaded && !(m.file in progress) && <Button variant="primary" onClick={() => download(m.file)}>Download</Button>}
            {m.downloaded && active !== m.file && <Button onClick={() => onSelect(m.file)}>Use</Button>}
            {m.downloaded && active !== m.file && <Button variant="danger" onClick={() => api.deleteModel(m.file).then(load)}>Delete</Button>}
          </div>
        </div>
      ))}
      {error && <p className="text-sm text-red-600">{error}</p>}
    </div>
  );
}
