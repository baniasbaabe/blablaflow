import { useEffect, useState } from "react";
import { api } from "../api";
import { Button, Card, HotkeyInput, Select, Toggle, useSettings } from "../ui";
import { GroqKey } from "./Groq";
import { ModelPicker } from "./Models";

const STEPS = ["Microphone", "Shortcut", "Speech model", "Cleanup", "Try it"];

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [s, update] = useSettings();
  const [step, setStep] = useState(0);
  const [mics, setMics] = useState<string[]>([]);
  const [modelReady, setModelReady] = useState(false);
  const [practice, setPractice] = useState("");
  useEffect(() => { api.listMics().then(setMics); }, []);
  useEffect(() => { if (s) api.listModels().then((ms) => setModelReady(ms.some((m) => m.file === s.model && m.downloaded))); }, [s, step]);
  if (!s) return null;

  const finish = async () => {
    await update({ onboarded: true });
    onDone();
  };

  const body = [
    <div key="mic" className="space-y-3">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">Pick the microphone blablaflow should listen to. A mic close to your mouth works best; Bluetooth earbuds often clip the first words.</p>
      <Select value={s.mic ?? ""} onChange={(v) => update({ mic: v || null })} options={[["", "System default"], ...mics.map((m) => [m, m] as [string, string])]} />
    </div>,
    <div key="hotkey" className="space-y-3">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">Hold this shortcut while you speak, let go to paste. Double-tap it to keep recording hands-free.</p>
      <HotkeyInput value={s.hotkeys.push_to_talk[0] ?? null} action="PushToTalk"
        onChange={(v) => v && update({ hotkeys: { ...s.hotkeys, push_to_talk: [v, ...s.hotkeys.push_to_talk.slice(1)] } })} />
    </div>,
    <div key="model" className="space-y-3">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">Speech is turned into text on your computer – your voice never leaves it. Download a model once.</p>
      <ModelPicker active={s.model} onSelect={(model) => { update({ model }); setModelReady(true); }} />
    </div>,
    <div key="groq" className="space-y-4">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">
        Groq removes filler words, fixes punctuation and formats your text in a fraction of a second. Only text is sent, never audio.
        You can skip this and get the raw transcript.
      </p>
      <GroqKey onModel={(groq_model) => update({ groq_model })} />
      <Toggle label="Context awareness" hint="Also send a little text around your cursor to Groq so names and style match. Password fields are never read."
        checked={s.context_awareness} onChange={(context_awareness) => update({ context_awareness })} />
    </div>,
    <div key="try" className="space-y-3">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">
        Click into the box, hold <span className="font-mono">{s.hotkeys.push_to_talk[0]}</span>, say something like
        "um so the meeting is at 5, actually 6pm", and let go.
      </p>
      <textarea className="h-32 w-full rounded-lg border border-neutral-300 p-3 text-sm dark:border-neutral-700 dark:bg-neutral-900"
        value={practice} onChange={(e) => setPractice(e.target.value)} placeholder="Your text appears here" />
    </div>,
  ];

  return (
    <div className="flex h-screen items-center justify-center bg-neutral-50 p-8 dark:bg-neutral-950">
      <Card className="w-full max-w-lg p-8">
        <div className="mb-6 flex gap-1.5">
          {STEPS.map((x, i) => <div key={x} className={`h-1 flex-1 rounded-full transition-colors duration-300 ease-out ${i <= step ? "bg-neutral-900 dark:bg-white" : "bg-neutral-200 dark:bg-neutral-800"}`} />)}
        </div>
        {/* Seen once: a short enter per step is welcome here. Re-keyed so @starting-style runs on every step. */}
        <div key={step} className="transition-[opacity,transform] duration-300 ease-out starting:opacity-0 motion-safe:starting:translate-y-1.5">
          <h1 className="mb-4 text-xl font-semibold">{STEPS[step]}</h1>
          {body[step]}
        </div>
        <div className="mt-8 flex justify-between">
          <Button variant="ghost" disabled={step === 0} onClick={() => setStep(step - 1)}>Back</Button>
          {step < STEPS.length - 1
            ? <Button variant="primary" disabled={step === 2 && !modelReady} onClick={() => setStep(step + 1)}>{step === 3 ? "Continue" : "Next"}</Button>
            : <Button variant="primary" onClick={finish}>Done</Button>}
        </div>
      </Card>
    </div>
  );
}
