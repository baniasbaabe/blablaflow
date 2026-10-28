import { test } from "node:test";
import assert from "node:assert/strict";
import { heatmapDays, isoDay } from "./heatmap.ts";

test("last cell is today, first cell is a Sunday", () => {
  const today = new Date(2026, 9, 28); // Wednesday
  const days = heatmapDays(today, 12, new Map([["2026-10-28", 40]]));
  assert.equal(days.at(-1)![0], "2026-10-28");
  assert.equal(days.at(-1)![1], 40);
  assert.equal(new Date(2026, 9, 28 - (days.length - 1)).getDay(), 0);
  assert.equal(days.length, 11 * 7 + 4);
});

test("isoDay uses local date", () => {
  assert.equal(isoDay(new Date(2026, 0, 5, 23, 30)), "2026-01-05");
});
