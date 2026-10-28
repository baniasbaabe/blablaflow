import { invoke } from "@tauri-apps/api/core";

export type CleanupLevel = "None" | "Light" | "Medium";
export type Tone = "Formal" | "Casual" | "VeryCasual" | "Excited";
export type Category = "Personal" | "Work" | "Email" | "Other";
export type Theme = "System" | "Light" | "Dark";
export type Mode = "PushToTalk" | "HandsFree" | "Command";
export type Action =
  | "PushToTalk" | "HandsFree" | "Command" | "Cancel" | "PasteLast" | "CopyLast" | "Scratchpad"
  | { Transform: number };

export interface Hotkeys {
  push_to_talk: string[];
  hands_free: string[];
  command: string[];
  cancel: string[];
  paste_last: string[];
  copy_last: string[];
  scratchpad: string[];
}

export interface Settings {
  hotkeys: Hotkeys;
  mic: string | null;
  language: string | null;
  model: string;
  use_gpu: boolean;
  gpu_device: number;
  groq_model: string;
  cleanup: CleanupLevel;
  styles: Record<Lowercase<Category>, Tone>;
  category_rules: { pattern: string; category: Category }[];
  smart_formatting: boolean;
  backtrack: boolean;
  file_tagging: boolean;
  context_awareness: boolean;
  context_excluded_apps: string[];
  bar_always_visible: boolean;
  sounds: boolean;
  bar_position: [number, number] | null;
  auto_transform: number | null;
  theme: Theme;
  onboarded: boolean;
}

export interface HistoryEntry {
  id: number;
  created_at: number;
  status: "Recording" | "Processing" | "Done" | "Failed" | "Interrupted";
  raw_text: string;
  final_text: string;
  app: string;
  category: string;
  language: string;
  duration_ms: number;
  words: number;
  audio_path: string | null;
  kind: "Dictation" | "Command" | "Transform";
}

export interface DictEntry { id: number; term: string; replacement: string | null }
export interface Snippet { id: number; cue: string; text: string }
export interface Transform { id: number; name: string; prompt: string; hotkey: string | null; samples: string[]; builtin: boolean }
export interface Note { id: number; title: string; body: string; updated_at: number }
export interface Insights {
  total_words: number;
  avg_wpm: number;
  words_this_week: number;
  streak_days: number;
  top_apps: [string, number][];
  daily: [string, number][];
}
export interface LocalModel { file: string; label: string; size_mb: number; downloaded: boolean }
export interface Gpu { id: number; name: string }

export type BarState =
  | { state: "Idle" }
  | { state: "Recording"; mode: Mode; elapsed_ms: number }
  | { state: "Processing" }
  | { state: "Message"; text: string; error: boolean };

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  listMics: () => invoke<string[]>("list_mics"),
  listModels: () => invoke<LocalModel[]>("list_models_local"),
  downloadModel: (file: string) => invoke<void>("download_model", { file }),
  deleteModel: (file: string) => invoke<void>("delete_model", { file }),
  listGpus: () => invoke<Gpu[]>("list_gpus"),
  groqKeySet: () => invoke<boolean>("groq_key_set"),
  setGroqKey: (key: string | null) => invoke<void>("set_groq_key", { key }),
  listGroqModels: () => invoke<string[]>("list_groq_models"),
  testGroq: () => invoke<string>("test_groq"),
  historyList: (query: string, offset = 0) => invoke<HistoryEntry[]>("history_list", { query, offset }),
  historyDelete: (id: number) => invoke<void>("history_delete", { id }),
  historyRetry: (id: number) => invoke<void>("history_retry", { id }),
  historyCopy: (id: number, raw: boolean) => invoke<void>("history_copy", { id, raw }),
  insights: () => invoke<Insights>("insights", { utcOffsetS: -new Date().getTimezoneOffset() * 60 }),
  dictList: () => invoke<DictEntry[]>("dict_list"),
  dictAdd: (term: string, replacement: string | null) => invoke<number>("dict_add", { term, replacement }),
  dictDelete: (id: number) => invoke<void>("dict_delete", { id }),
  snippetsList: () => invoke<Snippet[]>("snippets_list"),
  snippetAdd: (cue: string, text: string) => invoke<number>("snippet_add", { cue, text }),
  snippetUpdate: (snippet: Snippet) => invoke<void>("snippet_update", { snippet }),
  snippetDelete: (id: number) => invoke<void>("snippet_delete", { id }),
  transformsList: () => invoke<Transform[]>("transforms_list"),
  transformSave: (transform: Transform) => invoke<number>("transform_save", { transform }),
  transformDelete: (id: number) => invoke<void>("transform_delete", { id }),
  notesList: () => invoke<Note[]>("notes_list"),
  noteSave: (note: Note) => invoke<number>("note_save", { note }),
  noteDelete: (id: number) => invoke<void>("note_delete", { id }),
  exportItems: (kind: "dictionary" | "snippets") => invoke<string>("export_items", { kind }),
  importItems: (kind: "dictionary" | "snippets", json: string) => invoke<number>("import_items", { kind, json }),
  validateHotkey: (hotkey: string, action: Action) => invoke<string>("validate_hotkey", { hotkey, action }),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  hideBarFor: (minutes: number) => invoke<void>("hide_bar_for", { minutes }),
  setLanguage: (language: string | null) => invoke<void>("set_language", { language }),
  showMain: () => invoke<void>("show_main"),
};

/** Common Whisper languages offered in pickers; "auto" means detect. */
export const LANGUAGES: [string, string][] = [
  ["en", "English"], ["de", "German"], ["fr", "French"], ["es", "Spanish"], ["it", "Italian"],
  ["pt", "Portuguese"], ["nl", "Dutch"], ["pl", "Polish"], ["tr", "Turkish"], ["ru", "Russian"],
  ["uk", "Ukrainian"], ["ar", "Arabic"], ["hi", "Hindi"], ["zh", "Chinese"], ["ja", "Japanese"],
  ["ko", "Korean"], ["sv", "Swedish"], ["da", "Danish"], ["no", "Norwegian"], ["fi", "Finnish"],
];
