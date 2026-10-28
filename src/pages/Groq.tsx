import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { Button, inputClass } from "../ui";

/** API key entry + test. Calls onModel with the model Groq will use after a successful test. */
export function GroqKey({ onModel }: { onModel?: (m: string) => void }) {
  const [isSet, setIsSet] = useState(false);
  const [key, setKey] = useState("");
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  useEffect(() => { api.groqKeySet().then(setIsSet); }, []);

  const save = async () => {
    setStatus({ ok: true, text: "Testing…" });
    try {
      await api.setGroqKey(key);
      setIsSet(true);
      setKey("");
      const model = await api.testGroq();
      setStatus({ ok: true, text: `Works – using ${model}` });
      onModel?.(model);
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    }
  };

  return (
    <div>
      <div className="flex gap-2">
        <input type="password" className={inputClass} placeholder={isSet ? "Key saved – paste a new one to replace" : "gsk_…"}
          value={key} onChange={(e) => setKey(e.target.value)} />
        <Button variant="primary" disabled={!key.trim()} onClick={save}>Save</Button>
        {isSet && <Button onClick={() => api.setGroqKey(null).then(() => { setIsSet(false); setStatus(null); })}>Remove</Button>}
      </div>
      <p className="mt-2 text-xs text-neutral-500">
        Stored in Windows Credential Manager.{" "}
        <button className="underline" onClick={() => openUrl("https://console.groq.com/keys")}>Get a free key</button>
      </p>
      {status && <p className={`mt-2 text-sm ${status.ok ? "text-emerald-600" : "text-red-600"}`}>{status.text}</p>}
    </div>
  );
}
