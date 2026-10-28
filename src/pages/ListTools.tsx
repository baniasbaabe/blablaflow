import { useRef, useState } from "react";
import { api } from "../api";
import { Button } from "../ui";

/** Import/export buttons for dictionary and snippets (JSON files). */
export function ImportExport({ kind, onImported }: { kind: "dictionary" | "snippets"; onImported: () => void }) {
  const file = useRef<HTMLInputElement>(null);
  const [msg, setMsg] = useState("");
  const exportFile = async () => {
    const json = await api.exportItems(kind);
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([json], { type: "application/json" }));
    a.download = `blablaflow-${kind}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  const importFile = async (f: File) => {
    if (f.size > 3 * 1024 * 1024) return setMsg("File is larger than 3 MB");
    try {
      const n = await api.importItems(kind, await f.text());
      setMsg(`Imported ${n} item${n === 1 ? "" : "s"}`);
      onImported();
    } catch (e) {
      setMsg(String(e));
    }
  };
  return (
    <div className="flex items-center gap-2">
      {msg && <span className="text-xs text-neutral-500">{msg}</span>}
      <input ref={file} type="file" accept=".json,application/json" className="hidden"
        onChange={(e) => e.target.files?.[0] && importFile(e.target.files[0])} />
      <Button onClick={() => file.current?.click()}>Import</Button>
      <Button onClick={exportFile}>Export</Button>
    </div>
  );
}
