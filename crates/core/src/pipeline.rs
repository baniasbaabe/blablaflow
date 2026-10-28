//! The text side of a dictation: audio → transcript → cleanup → final text.
//! OS work (recording, pasting, storage) stays in the app; this is testable with a fake `Engine`.
use crate::audio::trim_silence;
use crate::category::Context;
use crate::db::{DictEntry, Snippet, Transform};
use crate::groq::Message;
use crate::prompt::{cleanup_messages, command_messages, transform_messages};
use crate::settings::{CleanupLevel, Settings};
use crate::text::{apply_corrections, expand_snippets, is_hallucination, sanitize_llm_output, strip_annotations, strip_wrappers};

/// Shorter speech than this (after trimming) is treated as an accidental press.
const MIN_SPEECH_SAMPLES: usize = 2_400; // 150 ms

pub trait Engine {
    /// Returns (text, detected language).
    fn transcribe(&self, samples: &[f32], language: Option<&str>, hint: &str) -> Result<(String, String), String>;
    fn chat(&self, messages: Vec<Message>) -> Result<String, String>;
}

pub struct Inputs<'a> {
    pub settings: &'a Settings,
    pub dict: &'a [DictEntry],
    pub snippets: &'a [Snippet],
    pub auto_transform: Option<&'a Transform>,
    pub ctx: &'a Context,
    pub has_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Nothing was said: paste nothing, keep no history.
    Empty,
    Done { raw: String, text: String, language: String, notice: Option<String> },
}

fn transcribe(samples: &[f32], inp: &Inputs, eng: &impl Engine) -> Result<Option<(String, String)>, String> {
    let speech = trim_silence(samples);
    if speech.len() < MIN_SPEECH_SAMPLES {
        return Ok(None);
    }
    let hint: Vec<&str> = inp.dict.iter().map(|d| d.replacement.as_deref().unwrap_or(&d.term)).collect();
    let (raw, lang) = eng.transcribe(speech, inp.settings.language.as_deref(), &hint.join(", "))?;
    let raw = strip_annotations(&raw);
    Ok((!is_hallucination(&raw)).then_some((raw, lang)))
}

pub fn run_dictation(samples: &[f32], inp: &Inputs, eng: &impl Engine) -> Result<Outcome, String> {
    let Some((raw, language)) = transcribe(samples, inp, eng)? else { return Ok(Outcome::Empty) };
    let (text, notice) = clean_text(&raw, inp, eng);
    Ok(Outcome::Done { raw, text, language, notice })
}

/// Cleanup, dictionary corrections, snippets and auto-transform on an existing transcript.
/// Returns the final text and an optional notice for the user.
pub fn clean_text(raw: &str, inp: &Inputs, eng: &impl Engine) -> (String, Option<String>) {
    let raw = raw.to_string();
    let mut text = raw.clone();
    let mut notice = None;
    if inp.settings.cleanup != CleanupLevel::None {
        if !inp.has_key {
            notice = Some("Cleanup skipped – no Groq API key".to_string());
        } else {
            match eng.chat(cleanup_messages(&raw, inp.ctx, inp.settings, inp.dict)) {
                Ok(out) => text = sanitize_llm_output(&raw, &out),
                Err(e) => notice = Some(format!("Cleanup skipped – {e}")),
            }
        }
    }
    text = expand_snippets(&apply_corrections(&text, inp.dict), inp.snippets);
    if let Some(t) = inp.auto_transform
        && inp.has_key
        && let Ok(out) = eng.chat(transform_messages(t, &text))
    {
        text = sanitize_llm_output(&text, &out);
    }
    (text, notice)
}

/// Command Mode: the speech is an instruction applied to `selection` (or used to write new text).
pub fn run_command(samples: &[f32], selection: Option<&str>, inp: &Inputs, eng: &impl Engine) -> Result<Outcome, String> {
    let Some((raw, language)) = transcribe(samples, inp, eng)? else { return Ok(Outcome::Empty) };
    if !inp.has_key {
        return Err("Command Mode needs a Groq API key".into());
    }
    let text = strip_wrappers(&raw, &eng.chat(command_messages(&raw, selection, inp.ctx))?);
    if text.is_empty() {
        return Err("Empty answer from Groq".into());
    }
    Ok(Outcome::Done { raw, text, language, notice: None })
}

pub fn run_transform(text: &str, t: &Transform, eng: &impl Engine) -> Result<String, String> {
    let out = strip_wrappers(text, &eng.chat(transform_messages(t, text))?);
    if out.is_empty() { Err("Empty answer from Groq".into()) } else { Ok(out) }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::category::{Category, Context};
    use crate::db::{DictEntry, Snippet, Transform};
    use crate::settings::{CleanupLevel, Settings};

    struct Fake {
        transcript: &'static str,
        chat: Result<&'static str, &'static str>,
        calls: RefCell<Vec<String>>,
    }

    impl Engine for Fake {
        fn transcribe(&self, _: &[f32], language: Option<&str>, hint: &str) -> Result<(String, String), String> {
            self.calls.borrow_mut().push(format!("stt lang={language:?} hint={hint}"));
            Ok((self.transcript.into(), "en".into()))
        }
        fn chat(&self, messages: Vec<crate::groq::Message>) -> Result<String, String> {
            self.calls.borrow_mut().push(format!("chat {}", messages[1].content));
            self.chat.map(String::from).map_err(String::from)
        }
    }

    fn fake(transcript: &'static str, chat: Result<&'static str, &'static str>) -> Fake {
        Fake { transcript, chat, calls: RefCell::new(vec![]) }
    }

    fn speech() -> Vec<f32> {
        (0..16_000).map(|i| 0.3 * (i as f32 * 0.1).sin()).collect()
    }

    fn ctx() -> Context {
        Context { exe: "notepad.exe".into(), title: String::new(), category: Category::Other, nearby_text: None }
    }

    fn inputs<'a>(s: &'a Settings, c: &'a Context, dict: &'a [DictEntry], snips: &'a [Snippet]) -> Inputs<'a> {
        Inputs { settings: s, dict, snippets: snips, auto_transform: None, ctx: c, has_key: true }
    }

    #[test]
    fn silent_job_is_empty_and_skips_engine() {
        let (s, c) = (Settings::default(), ctx());
        let f = fake("hi", Ok("Hi."));
        assert_eq!(run_dictation(&vec![0.0; 32_000], &inputs(&s, &c, &[], &[]), &f), Ok(Outcome::Empty));
        assert!(f.calls.borrow().is_empty());
    }

    #[test]
    fn hallucination_is_empty_and_skips_llm() {
        let (s, c) = (Settings::default(), ctx());
        let f = fake("Thank you.", Ok("Thank you."));
        assert_eq!(run_dictation(&speech(), &inputs(&s, &c, &[], &[]), &f), Ok(Outcome::Empty));
        assert_eq!(f.calls.borrow().len(), 1);
    }

    #[test]
    fn cleans_then_corrects_then_expands() {
        let (s, c) = (Settings::default(), ctx());
        let dict = [DictEntry { id: 1, term: "wisper".into(), replacement: Some("Wispr".into()) }];
        let snips = [Snippet { id: 1, cue: "my link".into(), text: "https://x.y".into() }];
        let f = fake("um wisper my link", Ok("Here is the text:\nWisper: my link."));
        let out = run_dictation(&speech(), &inputs(&s, &c, &dict, &snips), &f).unwrap();
        assert_eq!(
            out,
            Outcome::Done { raw: "um wisper my link".into(), text: "Wispr: https://x.y.".into(), language: "en".into(), notice: None }
        );
        assert!(f.calls.borrow()[0].contains("hint=Wispr"));
    }

    #[test]
    fn llm_error_falls_back_to_raw() {
        let (s, c) = (Settings::default(), ctx());
        let f = fake("hello there", Err("timeout"));
        let Outcome::Done { text, notice, .. } = run_dictation(&speech(), &inputs(&s, &c, &[], &[]), &f).unwrap() else { panic!() };
        assert_eq!(text, "hello there");
        assert_eq!(notice.as_deref(), Some("Cleanup skipped – timeout"));
    }

    #[test]
    fn no_key_or_level_none_skips_llm() {
        let c = ctx();
        let s = Settings { cleanup: CleanupLevel::None, ..Settings::default() };
        let f = fake("raw text", Ok("X"));
        let Outcome::Done { text, notice, .. } = run_dictation(&speech(), &inputs(&s, &c, &[], &[]), &f).unwrap() else { panic!() };
        assert_eq!((text.as_str(), notice), ("raw text", None));
        assert_eq!(f.calls.borrow().len(), 1);

        let s = Settings::default();
        let mut inp = inputs(&s, &c, &[], &[]);
        inp.has_key = false;
        let Outcome::Done { text, notice, .. } = run_dictation(&speech(), &inp, &f).unwrap() else { panic!() };
        assert_eq!(text, "raw text");
        assert_eq!(notice.as_deref(), Some("Cleanup skipped – no Groq API key"));
    }

    #[test]
    fn auto_transform_runs_last_and_failure_keeps_text() {
        let (s, c) = (Settings::default(), ctx());
        let t = Transform { id: 2, name: "Up".into(), prompt: "uppercase".into(), hotkey: None, samples: vec![], builtin: false };
        let f = fake("hi", Ok("HI."));
        let mut inp = inputs(&s, &c, &[], &[]);
        inp.auto_transform = Some(&t);
        let Outcome::Done { text, .. } = run_dictation(&speech(), &inp, &f).unwrap() else { panic!() };
        assert_eq!(text, "HI.");
        assert_eq!(f.calls.borrow().len(), 3);
        assert!(f.calls.borrow()[2].contains("<text>\nHI.\n</text>"));
    }

    #[test]
    fn command_needs_key_and_uses_selection() {
        let (s, c) = (Settings::default(), ctx());
        let f = fake("make it shorter", Ok("Short."));
        let inp = inputs(&s, &c, &[], &[]);
        let Outcome::Done { text, raw, .. } = run_command(&speech(), Some("A long sentence."), &inp, &f).unwrap() else { panic!() };
        assert_eq!((raw.as_str(), text.as_str()), ("make it shorter", "Short."));
        assert!(f.calls.borrow()[1].contains("<selection>"));
        let mut inp = inputs(&s, &c, &[], &[]);
        inp.has_key = false;
        assert!(run_command(&speech(), None, &inp, &f).is_err());
    }

    #[test]
    fn transform_strips_wrappers() {
        let t = Transform { id: 1, name: "Polish".into(), prompt: "p".into(), hotkey: None, samples: vec![], builtin: true };
        let f = fake("", Ok("Sure, here's the polished version:\nNice text."));
        assert_eq!(run_transform("nice txt", &t, &f), Ok("Nice text.".into()));
        let f = fake("", Ok("  "));
        assert!(run_transform("x", &t, &f).is_err());
    }
}
