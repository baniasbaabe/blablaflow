export function isoDay(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Days for a week-column heatmap: starts on a Sunday `weeks - 1` weeks back, ends today. */
export function heatmapDays(today: Date, weeks: number, byDay: Map<string, number>): [string, number][] {
  const count = (weeks - 1) * 7 + today.getDay() + 1;
  return Array.from({ length: count }, (_, n) => {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (count - 1 - n));
    return [isoDay(d), byDay.get(isoDay(d)) ?? 0];
  });
}
