import { useEffect, useState } from "react";
import { api, Insights } from "../api";
import { Card, Page } from "../ui";

const WEEKS = 12;

function Tile({ label, value }: { label: string; value: string | number }) {
  return (
    <Card>
      <div className="text-xs text-neutral-500">{label}</div>
      <div className="mt-1 text-2xl font-semibold tabular-nums">{value}</div>
    </Card>
  );
}

function isoDay(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export default function InsightsPage() {
  const [i, setI] = useState<Insights | null>(null);
  useEffect(() => { api.insights().then(setI); }, []);
  if (!i) return null;

  const byDay = new Map(i.daily);
  const max = Math.max(1, ...i.daily.map(([, w]) => w));
  const start = new Date();
  start.setDate(start.getDate() - (WEEKS * 7 - 1) - start.getDay());
  const days = Array.from({ length: WEEKS * 7 }, (_, n) => {
    const d = new Date(start);
    d.setDate(start.getDate() + n);
    return [isoDay(d), byDay.get(isoDay(d)) ?? 0] as const;
  });

  return (
    <Page title="Insights">
      <div className="mb-6 grid grid-cols-4 gap-3">
        <Tile label="Total words" value={i.total_words.toLocaleString()} />
        <Tile label="Words per minute" value={i.avg_wpm} />
        <Tile label="This week" value={i.words_this_week.toLocaleString()} />
        <Tile label="Day streak" value={i.streak_days} />
      </div>
      <Card className="mb-6">
        <div className="mb-3 text-sm font-medium">Last {WEEKS} weeks</div>
        <div className="grid grid-flow-col grid-rows-7 gap-1">
          {days.map(([d, w]) => (
            <div key={d} title={`${d}: ${w} words`} className="size-3.5 rounded-sm bg-neutral-900 dark:bg-white"
              style={{ opacity: w ? 0.15 + 0.85 * (w / max) : 0.06 }} />
          ))}
        </div>
      </Card>
      <Card>
        <div className="mb-3 text-sm font-medium">Top apps</div>
        {i.top_apps.length === 0 && <p className="text-sm text-neutral-500">No dictations yet.</p>}
        {i.top_apps.map(([app, w]) => (
          <div key={app} className="flex justify-between py-1 text-sm">
            <span>{app || "Unknown"}</span>
            <span className="tabular-nums text-neutral-500">{w.toLocaleString()} words</span>
          </div>
        ))}
      </Card>
    </Page>
  );
}
