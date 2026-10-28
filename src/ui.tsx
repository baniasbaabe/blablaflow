import { ReactNode, useEffect, useState } from "react";
import { Action, api, Settings } from "./api";

export function Page({ title, subtitle, children, actions }: { title: string; subtitle?: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="mx-auto max-w-3xl px-8 py-8">
      <div className="mb-6 flex items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          {subtitle && <p className="mt-1 text-sm text-neutral-500">{subtitle}</p>}
        </div>
        {actions}
      </div>
      {children}
    </div>
  );
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`rounded-xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900 ${className}`}>{children}</div>;
}

export function Button({ children, onClick, variant = "default", disabled, type = "button" }: {
  children: ReactNode; onClick?: () => void; variant?: "default" | "primary" | "danger" | "ghost"; disabled?: boolean; type?: "button" | "submit";
}) {
  const styles = {
    default: "border border-neutral-300 bg-white hover:bg-neutral-50 dark:border-neutral-700 dark:bg-neutral-800 dark:hover:bg-neutral-700",
    primary: "bg-neutral-900 text-white hover:bg-neutral-700 dark:bg-white dark:text-neutral-900 dark:hover:bg-neutral-200",
    danger: "text-red-600 hover:bg-red-50 dark:hover:bg-red-950",
    ghost: "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800",
  }[variant];
  return (
    <button type={type} onClick={onClick} disabled={disabled}
      className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-40 ${styles}`}>
      {children}
    </button>
  );
}

export const inputClass =
  "w-full rounded-lg border border-neutral-300 bg-white px-3 py-1.5 text-sm outline-none focus:border-neutral-500 dark:border-neutral-700 dark:bg-neutral-900";

export function Toggle({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex cursor-pointer items-start justify-between gap-4 py-2">
      <span>
        <span className="text-sm font-medium">{label}</span>
        {hint && <span className="block text-xs text-neutral-500">{hint}</span>}
      </span>
      <button type="button" role="switch" aria-checked={checked} onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors ${checked ? "bg-neutral-900 dark:bg-white" : "bg-neutral-300 dark:bg-neutral-700"}`}>
        <span className={`absolute top-0.5 size-4 rounded-full bg-white transition-all dark:bg-neutral-900 ${checked ? "left-4.5" : "left-0.5"}`} />
      </button>
    </label>
  );
}

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-2">
      <div>
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="text-xs text-neutral-500">{hint}</div>}
      </div>
      <div className="w-64 shrink-0">{children}</div>
    </div>
  );
}

export function Select<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <select className={inputClass} value={value} onChange={(e) => onChange(e.target.value as T)}>
      {options.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
    </select>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  return children ? <p className="mt-2 text-sm text-red-600">{children}</p> : null;
}

/** Settings loaded once and saved on every change. */
export function useSettings(): [Settings | null, (patch: Partial<Settings>) => Promise<void>] {
  const [s, setS] = useState<Settings | null>(null);
  useEffect(() => { api.getSettings().then(setS); }, []);
  const update = async (patch: Partial<Settings>) => {
    const next = { ...(s ?? (await api.getSettings())), ...patch };
    setS(next);
    await api.saveSettings(next);
  };
  return [s, update];
}

const CODE_NAMES: Record<string, string> = {
  ControlLeft: "Ctrl", ControlRight: "Ctrl", MetaLeft: "Win", MetaRight: "Win", AltLeft: "Alt", AltRight: "Alt",
  ShiftLeft: "Shift", ShiftRight: "Shift", Space: "Space", Escape: "Escape", CapsLock: "CapsLock",
};

function keyName(code: string): string | null {
  if (CODE_NAMES[code]) return CODE_NAMES[code];
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  return null;
}

/** Records a key combo (or mouse middle/side button). Global hotkeys are paused while recording. */
export function HotkeyInput({ value, action, onChange }: { value: string | null; action: Action; onChange: (v: string | null) => void }) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!recording) return;
    api.setPaused(true);
    const keys = new Set<string>();
    const held = new Set<string>();
    const finish = async () => {
      setRecording(false);
      const combo = [...keys].join("+");
      try {
        onChange(await api.validateHotkey(combo, action));
        setError("");
      } catch (e) {
        setError(`${combo}: ${e}`);
      }
    };
    const down = (e: KeyboardEvent) => {
      e.preventDefault();
      const k = keyName(e.code);
      if (k) { keys.add(k); held.add(k); }
    };
    const up = (e: KeyboardEvent) => {
      e.preventDefault();
      const k = keyName(e.code);
      if (k) held.delete(k);
      if (held.size === 0 && keys.size > 0) finish();
    };
    const mouse = (e: MouseEvent) => {
      const b = { 1: 3, 3: 4, 4: 5 }[e.button];
      if (!b) return;
      e.preventDefault();
      keys.add(`Mouse${b}`);
      finish();
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("mousedown", mouse, true);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
      window.removeEventListener("mousedown", mouse, true);
      api.setPaused(false);
    };
  }, [recording]);

  return (
    <div>
      <div className="flex items-center gap-2">
        <button type="button" onClick={() => setRecording(true)}
          className={`${inputClass} text-left font-mono ${recording ? "border-neutral-900 dark:border-white" : ""}`}>
          {recording ? "Press keys…" : value || "Not set"}
        </button>
        {value && !recording && <Button variant="ghost" onClick={() => onChange(null)}>✕</Button>}
      </div>
      {error && <p className="mt-1 text-xs text-red-600">{error}</p>}
    </div>
  );
}
