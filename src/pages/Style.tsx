import { useState } from "react";
import { Category, CleanupLevel, Tone } from "../api";
import { Button, Card, Page, Row, Select, Toggle, inputClass, useSettings } from "../ui";

const TONES: Record<Lowercase<Category>, [Tone, string][]> = {
  personal: [["Formal", "Formal"], ["Casual", "Casual"], ["VeryCasual", "Very casual"]],
  work: [["Formal", "Formal"], ["Casual", "Casual"], ["Excited", "Excited"]],
  email: [["Formal", "Formal"], ["Casual", "Casual"], ["Excited", "Excited"]],
  other: [["Formal", "Formal"], ["Casual", "Casual"], ["Excited", "Excited"]],
};
const EXAMPLES: Record<Tone, string> = {
  Formal: "Hey, are you free for lunch tomorrow? Let's do 12 if that works for you.",
  Casual: "Hey are you free for lunch tomorrow? Let's do 12 if that works for you",
  VeryCasual: "hey are you free for lunch tomorrow? let's do 12 if that works for you",
  Excited: "Hey, are you free for lunch tomorrow? Let's do 12 if that works for you!",
};
const CATEGORIES: [Lowercase<Category>, string, string][] = [
  ["personal", "Personal messages", "WhatsApp, Telegram, Signal…"],
  ["work", "Work messages", "Slack, Teams, Notion, code editors…"],
  ["email", "Email", "Outlook, Gmail…"],
  ["other", "Everything else", ""],
];

export default function Style() {
  const [s, update] = useSettings();
  const [pattern, setPattern] = useState("");
  const [category, setCategory] = useState<Category>("Work");
  if (!s) return null;

  return (
    <Page title="Style" subtitle="How much blablaflow edits, and how it sounds in each kind of app.">
      <Card className="mb-6">
        <Row label="Cleanup" hint="None keeps Whisper's raw text and sends nothing to Groq.">
          <Select<CleanupLevel> value={s.cleanup} onChange={(cleanup) => update({ cleanup })}
            options={[["None", "None"], ["Light", "Light – remove fillers, fix grammar"], ["Medium", "Medium – may rephrase"]]} />
        </Row>
        <Toggle label="Smart formatting" hint='"new line", "new paragraph", "period", spoken lists' checked={s.smart_formatting} onChange={(smart_formatting) => update({ smart_formatting })} />
        <Toggle label="Backtrack" hint={'"…at 5, actually 6" becomes "…at 6"'} checked={s.backtrack} onChange={(backtrack) => update({ backtrack })} />
        <Toggle label="File tagging in code editors" hint={'In Cursor, VS Code and others, "at main dot py" becomes @main.py'} checked={s.file_tagging} onChange={(file_tagging) => update({ file_tagging })} />
      </Card>

      <Card className="mb-6">
        {CATEGORIES.map(([key, label, hint]) => (
          <div key={key} className="border-b border-neutral-100 py-2 last:border-0 dark:border-neutral-800">
            <Row label={label} hint={hint}>
              <Select<Tone> value={s.styles[key]} options={TONES[key]} onChange={(t) => update({ styles: { ...s.styles, [key]: t } })} />
            </Row>
            <p className="pb-1 text-xs text-neutral-500 italic">{EXAMPLES[s.styles[key]]}</p>
          </div>
        ))}
      </Card>

      <Card>
        <div className="mb-1 text-sm font-medium">Your app rules</div>
        <p className="mb-3 text-xs text-neutral-500">Matched against the app's file name or window title (e.g. "slack", "gmail"). Yours win over the built-in list.</p>
        {s.category_rules.map((r, i) => (
          <div key={i} className="flex items-center justify-between py-1 text-sm">
            <span><span className="font-mono">{r.pattern}</span> → {r.category}</span>
            <Button variant="danger" onClick={() => update({ category_rules: s.category_rules.filter((_, j) => j !== i) })}>Remove</Button>
          </div>
        ))}
        <div className="mt-2 flex gap-2">
          <input className={inputClass} placeholder="App or site, e.g. linear" value={pattern} onChange={(e) => setPattern(e.target.value)} />
          <div className="w-40 shrink-0">
            <Select<Category> value={category} onChange={setCategory} options={[["Personal", "Personal"], ["Work", "Work"], ["Email", "Email"], ["Other", "Other"]]} />
          </div>
          <Button variant="primary" disabled={!pattern.trim()} onClick={() => {
            update({ category_rules: [...s.category_rules, { pattern: pattern.trim().toLowerCase(), category }] });
            setPattern("");
          }}>Add</Button>
        </div>
      </Card>
    </Page>
  );
}
