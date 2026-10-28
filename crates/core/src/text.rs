use std::collections::HashMap;

use regex::{Captures, Regex};

use crate::db::{DictEntry, Snippet};

/// Known Whisper outputs on silence/noise, compared without trailing punctuation.
const HALLUCINATIONS: &[&str] = &[
    "thank you",
    "thanks for watching",
    "you",
    "thank you for watching",
    "vielen dank",
    "untertitel im auftrag des zdf, 2017",
    "untertitel der amara.org-community",
];

/// Case-insensitive, whole-word replacement of `pairs` (key → value); longest key wins.
fn replace_words(text: &str, pairs: &[(&str, &str)]) -> String {
    let mut pairs: Vec<_> = pairs.iter().filter(|(k, _)| !k.trim().is_empty()).collect();
    if pairs.is_empty() {
        return text.to_string();
    }
    pairs.sort_by_key(|(k, _)| std::cmp::Reverse(k.chars().count()));
    let edge = |c: Option<char>| if c.is_some_and(char::is_alphanumeric) { r"\b" } else { "" };
    let alts: Vec<String> = pairs
        .iter()
        .map(|(k, _)| format!("{}{}{}", edge(k.chars().next()), regex::escape(k), edge(k.chars().last())))
        .collect();
    let re = Regex::new(&format!("(?iu){}", alts.join("|"))).expect("escaped pattern");
    let map: HashMap<String, &str> = pairs.iter().map(|(k, v)| (k.to_lowercase(), *v)).collect();
    re.replace_all(text, |c: &Captures| {
        let m = &c[0];
        map.get(&m.to_lowercase()).copied().unwrap_or(m).to_string()
    })
    .into_owned()
}

pub fn expand_snippets(text: &str, snippets: &[Snippet]) -> String {
    // A cue spoken as the whole utterance: drop the punctuation the LLM added.
    let bare = text.trim().trim_end_matches(['.', '!', '?', ',']).to_lowercase();
    if let Some(s) = snippets.iter().find(|s| s.cue.trim().to_lowercase() == bare) {
        return s.text.clone();
    }
    let pairs: Vec<(&str, &str)> = snippets.iter().map(|s| (s.cue.as_str(), s.text.as_str())).collect();
    replace_words(text, &pairs)
}

pub fn apply_corrections(text: &str, dict: &[DictEntry]) -> String {
    let pairs: Vec<(&str, &str)> =
        dict.iter().filter_map(|d| d.replacement.as_deref().map(|r| (d.term.as_str(), r))).collect();
    replace_words(text, &pairs)
}

pub fn word_count(text: &str) -> i64 {
    text.split_whitespace().count() as i64
}

pub fn is_hallucination(text: &str) -> bool {
    if !text.chars().any(char::is_alphanumeric) {
        return true;
    }
    let t = text.trim().trim_end_matches(|c: char| c.is_ascii_punctuation()).to_lowercase();
    HALLUCINATIONS.contains(&t.as_str())
}

/// Remove a chatty preamble, code fences and surrounding quotes an LLM sometimes adds.
pub fn strip_wrappers(out: &str) -> String {
    static PREAMBLE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(?i)^\s*(here('s| is)|sure|okay)[^\n]*:\s*\n").unwrap());
    let mut s = PREAMBLE.replace(out, "").trim().to_string();
    if let Some(inner) = s.strip_prefix("```").and_then(|r| r.strip_suffix("```")) {
        // Drop an optional language tag on the opening fence line.
        s = inner.split_once('\n').map_or(inner, |(_, body)| body).trim().to_string();
    }
    for q in ['"', '“'] {
        let close = if q == '“' { '”' } else { q };
        if s.len() > 1 && s.starts_with(q) && s.ends_with(close) {
            s = s[q.len_utf8()..s.len() - close.len_utf8()].trim().to_string();
        }
    }
    s
}

/// `strip_wrappers`, then fall back to `raw` on empty or runaway output (the model answered instead of cleaning).
pub fn sanitize_llm_output(raw: &str, out: &str) -> String {
    let s = strip_wrappers(out);
    let n = raw.chars().count();
    if s.is_empty() || s.chars().count() > (3 * n).max(n + 200) {
        return raw.to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DictEntry, Snippet};

    fn snip(cue: &str, text: &str) -> Snippet {
        Snippet { id: 0, cue: cue.into(), text: text.into() }
    }

    #[test]
    fn snippet_whole_word_ci() {
        let s = [snip("my calendar", "https://cal.com/me")];
        assert_eq!(expand_snippets("Book a slot: My Calendar.", &s), "Book a slot: https://cal.com/me.");
        assert_eq!(expand_snippets("mycalendar", &s), "mycalendar");
    }

    #[test]
    fn snippet_whole_utterance_drops_added_punctuation() {
        let s = [snip("my address", "Main St 1\n12345 Berlin")];
        assert_eq!(expand_snippets("My address.", &s), "Main St 1\n12345 Berlin");
    }

    #[test]
    fn snippet_umlaut_case() {
        let s = [snip("grüße", "Viele Grüße,\nBanias")];
        assert_eq!(expand_snippets("Liebe Grüße", &s), "Liebe Viele Grüße,\nBanias");
    }

    #[test]
    fn snippet_longest_wins() {
        let s = [snip("intro", "SHORT"), snip("intro long", "LONG")];
        assert_eq!(expand_snippets("intro long please", &s), "LONG please");
    }

    #[test]
    fn snippet_text_with_dollar_is_literal() {
        let s = [snip("price", "$1 and $2")];
        assert_eq!(expand_snippets("price", &s), "$1 and $2");
    }

    #[test]
    fn corrections_apply() {
        let d = [
            DictEntry { id: 0, term: "wisper".into(), replacement: Some("Wispr".into()) },
            DictEntry { id: 0, term: "Kubernetes".into(), replacement: None },
        ];
        assert_eq!(apply_corrections("I use Wisper and wispers.", &d), "I use Wispr and wispers.");
    }

    #[test]
    fn word_count_unicode() {
        assert_eq!(word_count("  Grüße  an\tdich\n"), 3);
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn is_hallucination_cases() {
        for t in ["", "  ", "...", "Thank you.", "you", "Vielen Dank.", "Thanks for watching!", " thank you "] {
            assert!(is_hallucination(t), "{t:?}");
        }
        assert!(!is_hallucination("Thank you for the update, Marcus."));
        assert!(!is_hallucination("Yes."));
    }

    #[test]
    fn sanitize_strips_preamble() {
        assert_eq!(sanitize_llm_output("hello", "Here is the cleaned text:\nHello."), "Hello.");
        assert_eq!(sanitize_llm_output("hello", "Sure! Here's your text:\n\nHello."), "Hello.");
    }

    #[test]
    fn sanitize_strips_fences_and_quotes() {
        assert_eq!(sanitize_llm_output("hi", "```\nHi.\n```"), "Hi.");
        assert_eq!(sanitize_llm_output("hi", "\"Hi.\""), "Hi.");
        assert_eq!(sanitize_llm_output("hi", "  Hi.  "), "Hi.");
    }

    #[test]
    fn sanitize_falls_back_when_runaway() {
        let raw = "short text";
        assert_eq!(sanitize_llm_output(raw, &"x".repeat(500)), raw);
        assert_eq!(sanitize_llm_output(raw, "   "), raw);
        let ok = "y".repeat(200);
        assert_eq!(sanitize_llm_output(raw, &ok), ok);
    }
}
