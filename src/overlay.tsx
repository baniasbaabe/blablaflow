import { ReactNode, useEffect, useRef, useState } from "react";
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

/** One stacked layer per state; switching crossfades with a slight scale instead of resizing the pill. */
function Layer({ show, children }: { show: boolean; children: ReactNode }) {
  return (
    <div
      aria-hidden={!show}
      className={`absolute inset-0 flex items-center justify-center transition-[opacity,transform] ease-out ${
        show ? "scale-100 opacity-100 duration-150" : "pointer-events-none opacity-0 duration-100 motion-safe:scale-[0.92]"
      }`}
    >
      {children}
    </div>
  );
}

const pill = "flex h-9 items-center justify-center gap-1 rounded-full bg-neutral-900/90 text-white shadow-lg ring-1 ring-white/10";

function Bar() {
  const [bar, setBar] = useState<BarState>({ state: "Idle" });
  const [levels, setLevels] = useState<number[]>(Array(BARS).fill(0));
  const prev = useRef<BarState["state"]>("Idle");
  // The last recording/message stays rendered while its layer fades out.
  const [rec, setRec] = useState<Extract<BarState, { state: "Recording" }>>({ state: "Recording", mode: "PushToTalk", elapsed_ms: 0 });
  const [msg, setMsg] = useState<Extract<BarState, { state: "Message" }>>({ state: "Message", text: "", error: false });

  useEffect(() => {
    const un1 = listen<BarState>("bar", async (e) => {
      const next = e.payload;
      if (next.state !== prev.current && (next.state === "Recording" || prev.current === "Recording")) {
        const { sounds } = await api.getSettings();
        if (sounds) blip(next.state === "Recording" ? 660 : 440);
      }
      if (next.state === "Recording" && prev.current !== "Recording") setLevels(Array(BARS).fill(0));
      prev.current = next.state;
      if (next.state === "Recording") setRec(next);
      if (next.state === "Message") setMsg(next);
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
    if (e.button === 0) api.barDragStart().then(() => getCurrentWindow().startDragging());
  };
  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    openMenu();
  };

  return (
    <div className="relative h-screen w-screen cursor-grab select-none active:cursor-grabbing" onMouseDown={onMouseDown} onContextMenu={onContextMenu}>
      <Layer show={bar.state === "Idle"}>
        <div className="h-1.5 w-10 rounded-full bg-neutral-900/70 ring-1 ring-white/30 transition-transform duration-150 ease-out hover:scale-x-125" />
      </Layer>
      <Layer show={bar.state === "Recording"}>
        <div className={`${pill} px-4`}>
          {rec.mode === "Command" && <span className="mr-1 text-xs font-medium text-amber-300">Command</span>}
          {levels.map((l, i) => (
            // scaleY instead of height: no layout work on every level update.
            <span key={i} className="h-5 w-1 rounded-full bg-white transition-transform duration-75 ease-out"
              style={{ transform: `scaleY(${0.2 + Math.min(1, l * 12) * 0.8})` }} />
          ))}
          {rec.elapsed_ms >= 10_000 && <span className="ml-2 text-xs tabular-nums text-white/70">{fmt(rec.elapsed_ms)}</span>}
        </div>
      </Layer>
      <Layer show={bar.state === "Processing"}>
        <div className={`${pill} w-20`}>
          {[0, 1, 2].map((i) => (
            <span key={i} className="processing-dot size-1.5 rounded-full bg-white" style={{ animationDelay: `${i * 150}ms` }} />
          ))}
        </div>
      </Layer>
      <Layer show={bar.state === "Message"}>
        <div className={`${pill} max-w-[216px] px-4 text-xs ${msg.error ? "text-red-300" : ""}`}>
          <span className="truncate" title={msg.text}>{msg.text}</span>
        </div>
      </Layer>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Bar />);
