import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckMenuItem, Menu, MenuItem, Submenu } from "@tauri-apps/api/menu";
import { api, BarState, LANGUAGES } from "./api";
import "./index.css";

const BARS = 7;
const MESSAGE_MS = 3000;

function blip(freq: number) {
  const ctx = new AudioContext();
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  gain.gain.setValueAtTime(0.08, ctx.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.06);
  osc.frequency.value = freq;
  osc.connect(gain).connect(ctx.destination);
  osc.start();
  osc.stop(ctx.currentTime + 0.06);
  osc.onended = () => ctx.close();
}

function fmt(ms: number) {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

async function openMenu() {
  const s = await api.getSettings();
  const lang = (code: string | null, label: string) =>
    CheckMenuItem.new({ text: label, checked: s.language === code, action: () => api.setLanguage(code) });
  const languages = await Submenu.new({
    text: "Language",
    items: [await lang(null, "Auto-detect"), ...(await Promise.all(LANGUAGES.map(([c, l]) => lang(c, l))))],
  });
  const menu = await Menu.new({
    items: [
      languages,
      await MenuItem.new({ text: "Hide for 1 hour", action: () => api.hideBarFor(60) }),
      await MenuItem.new({ text: "Open blablaflow", action: () => api.showMain() }),
    ],
  });
  await menu.popup();
}

function Bar() {
  const [bar, setBar] = useState<BarState>({ state: "Idle" });
  const [levels, setLevels] = useState<number[]>(Array(BARS).fill(0));
  const prev = useRef<BarState["state"]>("Idle");

  useEffect(() => {
    const un1 = listen<BarState>("bar", async (e) => {
      const next = e.payload;
      if (next.state !== prev.current && (next.state === "Recording" || prev.current === "Recording")) {
        const { sounds } = await api.getSettings();
        if (sounds) blip(next.state === "Recording" ? 660 : 440);
      }
      prev.current = next.state;
      setBar(next);
    });
    const un2 = listen<number>("level", (e) => setLevels((l) => [...l.slice(1), e.payload]));
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (bar.state !== "Message") return;
    const t = setTimeout(() => setBar({ state: "Idle" }), MESSAGE_MS);
    return () => clearTimeout(t);
  }, [bar]);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button === 0) getCurrentWindow().startDragging();
  };
  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    openMenu();
  };

  const pill = "flex items-center justify-center gap-1 rounded-full bg-neutral-900/90 text-white shadow-lg ring-1 ring-white/10";
  let body;
  switch (bar.state) {
    case "Recording":
      body = (
        <div className={`${pill} h-9 px-4`}>
          {bar.mode === "Command" && <span className="mr-1 text-xs font-medium text-amber-300">Command</span>}
          {levels.map((l, i) => (
            <span key={i} className="w-1 rounded-full bg-white" style={{ height: `${4 + Math.min(1, l * 12) * 18}px` }} />
          ))}
          {bar.elapsed_ms >= 10_000 && <span className="ml-2 text-xs tabular-nums text-white/70">{fmt(bar.elapsed_ms)}</span>}
        </div>
      );
      break;
    case "Processing":
      body = (
        <div className={`${pill} h-9 w-24`}>
          {[0, 1, 2].map((i) => (
            <span key={i} className="size-1.5 animate-pulse rounded-full bg-white" style={{ animationDelay: `${i * 150}ms` }} />
          ))}
        </div>
      );
      break;
    case "Message":
      body = (
        <div className={`${pill} h-9 max-w-[216px] px-4 text-xs ${bar.error ? "text-red-300" : ""}`}>
          <span className="truncate" title={bar.text}>{bar.text}</span>
        </div>
      );
      break;
    default:
      body = <div className="h-1.5 w-10 rounded-full bg-neutral-900/70 ring-1 ring-white/30" />;
  }

  return (
    <div className="flex h-screen w-screen cursor-grab items-center justify-center select-none" onMouseDown={onMouseDown} onContextMenu={onContextMenu}>
      {body}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Bar />);
