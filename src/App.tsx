import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, Settings } from "./api";
import Home from "./pages/Home";
import InsightsPage from "./pages/Insights";
import Dictionary from "./pages/Dictionary";
import Snippets from "./pages/Snippets";
import Style from "./pages/Style";
import Transforms from "./pages/Transforms";
import Scratchpad from "./pages/Scratchpad";
import SettingsPage from "./pages/Settings";
import Onboarding from "./pages/Onboarding";

const PAGES = {
  home: ["Home", Home],
  insights: ["Insights", InsightsPage],
  dictionary: ["Dictionary", Dictionary],
  snippets: ["Snippets", Snippets],
  style: ["Style", Style],
  transforms: ["Transforms", Transforms],
  scratchpad: ["Scratchpad", Scratchpad],
  settings: ["Settings", SettingsPage],
} as const;
type PageId = keyof typeof PAGES;

function applyTheme(theme: Settings["theme"]) {
  const dark = theme === "Dark" || (theme === "System" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

export default function App() {
  const [page, setPage] = useState<PageId>("home");
  const [onboarded, setOnboarded] = useState<boolean | null>(null);

  useEffect(() => {
    const load = () => api.getSettings().then((s) => { applyTheme(s.theme); setOnboarded(s.onboarded); });
    load();
    const media = matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", load);
    const un = listen<string>("navigate", (e) => setPage(e.payload as PageId));
    window.addEventListener("settings-changed", load);
    return () => {
      media.removeEventListener("change", load);
      un.then((f) => f());
      window.removeEventListener("settings-changed", load);
    };
  }, []);

  if (onboarded === null) return null;
  if (!onboarded) return <Onboarding onDone={() => setOnboarded(true)} />;

  const Current = PAGES[page][1];
  return (
    <div className="flex h-screen">
      <nav className="flex w-52 shrink-0 flex-col gap-0.5 border-r border-neutral-200 bg-neutral-50 p-3 dark:border-neutral-800 dark:bg-neutral-950">
        <div className="mb-4 px-3 pt-2 text-lg font-semibold tracking-tight">blablaflow</div>
        {(Object.keys(PAGES) as PageId[]).map((id) => (
          <button key={id} onClick={() => setPage(id)}
            className={`rounded-lg px-3 py-1.5 text-left text-sm transition-[background-color,color,transform] duration-100 ease-out motion-safe:active:scale-[0.98] ${page === id
              ? "bg-white font-medium shadow-sm ring-1 ring-neutral-200 dark:bg-neutral-800 dark:ring-neutral-700"
              : "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-400 dark:hover:bg-neutral-900"}`}>
            {PAGES[id][0]}
          </button>
        ))}
      </nav>
      <main className="flex-1 overflow-y-auto">
        <Current />
      </main>
    </div>
  );
}
