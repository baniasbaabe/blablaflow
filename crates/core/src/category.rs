use serde::{Deserialize, Serialize};

use crate::settings::CategoryRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Personal,
    Work,
    Email,
    Other,
}

/// What we know about where the text will go.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Context {
    /// Lowercased exe file name, e.g. "slack.exe".
    pub exe: String,
    pub title: String,
    pub category: Category,
    /// Text around the cursor (≤ 1500 chars), only when context awareness is on.
    pub nearby_text: Option<String>,
}

/// Built-in rules, matched as lowercase substrings of the exe name or window title.
pub fn default_rules() -> Vec<CategoryRule> {
    let rule = |p: &str, c| CategoryRule { pattern: p.into(), category: c };
    let mut r = Vec::new();
    for p in ["outlook", "thunderbird", "gmail", "outlook.live", "mail -", "proton mail"] {
        r.push(rule(p, Category::Email));
    }
    for p in ["slack", "teams", "discord", "zoom", "notion", "linear", "jira", "code.exe", "cursor", " - google docs"] {
        r.push(rule(p, Category::Work));
    }
    for p in ["whatsapp", "telegram", "signal", "messenger"] {
        r.push(rule(p, Category::Personal));
    }
    r
}

/// First matching rule wins; callers put user rules before `default_rules()`.
pub fn classify(exe: &str, title: &str, rules: &[CategoryRule]) -> Category {
    let (exe, title) = (exe.to_lowercase(), title.to_lowercase());
    rules
        .iter()
        .find(|r| {
            let p = r.pattern.to_lowercase();
            !p.is_empty() && (exe.contains(&p) || title.contains(&p))
        })
        .map_or(Category::Other, |r| r.category)
}

pub fn is_code_editor(exe: &str) -> bool {
    const EDITORS: &[&str] = &[
        "code.exe", "code - insiders.exe", "cursor.exe", "windsurf.exe", "zed.exe", "idea64.exe", "pycharm64.exe",
        "webstorm64.exe", "goland64.exe", "rider64.exe", "clion64.exe", "rustrover64.exe", "phpstorm64.exe",
    ];
    EDITORS.contains(&exe.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_defaults() {
        let r = default_rules();
        assert_eq!(classify("slack.exe", "general - Acme", &r), Category::Work);
        assert_eq!(classify("chrome.exe", "Inbox (3) - me@x.com - Gmail - Google Chrome", &r), Category::Email);
        assert_eq!(classify("whatsapp.exe", "WhatsApp", &r), Category::Personal);
        assert_eq!(classify("OUTLOOK.EXE", "Inbox", &r), Category::Email);
        assert_eq!(classify("notepad.exe", "Untitled - Notepad", &r), Category::Other);
    }

    #[test]
    fn user_rules_come_first() {
        let mut r = vec![CategoryRule { pattern: "slack".into(), category: Category::Personal }];
        r.extend(default_rules());
        assert_eq!(classify("slack.exe", "", &r), Category::Personal);
    }

    #[test]
    fn code_editors() {
        assert!(is_code_editor("Code.exe"));
        assert!(is_code_editor("cursor.exe"));
        assert!(is_code_editor("pycharm64.exe"));
        assert!(!is_code_editor("notepad.exe"));
    }
}
