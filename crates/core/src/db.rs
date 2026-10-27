use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

const MAX_TERM: usize = 60;
const MAX_SNIPPET_TEXT: usize = 4000;
const MAX_CUSTOM_TRANSFORMS: i64 = 8;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("too long")]
    TooLong,
    #[error("already exists")]
    Duplicate,
    #[error("limit reached")]
    Limit,
    #[error("built-in items cannot be deleted")]
    Builtin,
}

pub type Result<T> = std::result::Result<T, DbError>;

macro_rules! text_enum {
    ($name:ident { $($v:ident),+ }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $($v),+ }
        impl $name {
            fn as_str(self) -> &'static str { match self { $(Self::$v => stringify!($v)),+ } }
            fn parse(s: &str) -> Self {
                match s { $(stringify!($v) => Self::$v,)+ _ => panic!("bad {} in db: {s}", stringify!($name)) }
            }
        }
    };
}
text_enum!(Status { Recording, Processing, Done, Failed, Interrupted });
text_enum!(Kind { Dictation, Command, Transform });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    /// Unix milliseconds.
    pub created_at: i64,
    pub status: Status,
    pub raw_text: String,
    pub final_text: String,
    pub app: String,
    pub category: String,
    pub language: String,
    pub duration_ms: i64,
    pub words: i64,
    pub audio_path: Option<String>,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictEntry {
    pub id: i64,
    pub term: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snippet {
    pub id: i64,
    pub cue: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub id: i64,
    pub name: String,
    pub prompt: String,
    pub hotkey: Option<String>,
    pub samples: Vec<String>,
    pub builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub updated_at: i64,
}

pub const POLISH_PROMPT: &str = "Polish this text: fix grammar, spelling and punctuation, and improve clarity and flow. Keep the meaning, language and tone.";

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE history (
    id INTEGER PRIMARY KEY, created_at INTEGER NOT NULL, status TEXT NOT NULL,
    raw_text TEXT NOT NULL, final_text TEXT NOT NULL, app TEXT NOT NULL, category TEXT NOT NULL,
    language TEXT NOT NULL, duration_ms INTEGER NOT NULL, words INTEGER NOT NULL,
    audio_path TEXT, kind TEXT NOT NULL
);
CREATE INDEX history_created ON history(created_at);
CREATE TABLE dictionary (id INTEGER PRIMARY KEY, term TEXT NOT NULL, replacement TEXT);
CREATE TABLE snippets (id INTEGER PRIMARY KEY, cue TEXT NOT NULL, text TEXT NOT NULL);
CREATE TABLE transforms (
    id INTEGER PRIMARY KEY, name TEXT NOT NULL, prompt TEXT NOT NULL, hotkey TEXT,
    samples TEXT NOT NULL, builtin INTEGER NOT NULL
);
CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL, body TEXT NOT NULL, updated_at INTEGER NOT NULL);
"#];

pub struct Db {
    conn: Connection,
}

fn history_row(r: &Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: r.get(0)?,
        created_at: r.get(1)?,
        status: Status::parse(&r.get::<_, String>(2)?),
        raw_text: r.get(3)?,
        final_text: r.get(4)?,
        app: r.get(5)?,
        category: r.get(6)?,
        language: r.get(7)?,
        duration_ms: r.get(8)?,
        words: r.get(9)?,
        audio_path: r.get(10)?,
        kind: Kind::parse(&r.get::<_, String>(11)?),
    })
}

const HISTORY_COLS: &str =
    "id, created_at, status, raw_text, final_text, app, category, language, duration_ms, words, audio_path, kind";

fn too_long(s: &str, max: usize) -> Result<()> {
    if s.chars().count() > max { Err(DbError::TooLong) } else { Ok(()) }
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Db> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(sql)?;
            if i == 0 {
                conn.execute(
                    "INSERT INTO transforms (name, prompt, hotkey, samples, builtin) VALUES ('Polish', ?1, 'Win+Alt+1', '[]', 1)",
                    [POLISH_PROMPT],
                )?;
            }
            conn.pragma_update(None, "user_version", i as i64 + 1)?;
        }
        Ok(Db { conn })
    }

    // ---- history

    pub fn insert_history(&self, e: &HistoryEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (created_at, status, raw_text, final_text, app, category, language, duration_ms, words, audio_path, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![e.created_at, e.status.as_str(), e.raw_text, e.final_text, e.app, e.category, e.language,
                    e.duration_ms, e.words, e.audio_path, e.kind.as_str()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_history(&self, e: &HistoryEntry) -> Result<()> {
        self.conn.execute(
            "UPDATE history SET created_at=?2, status=?3, raw_text=?4, final_text=?5, app=?6, category=?7, language=?8,
             duration_ms=?9, words=?10, audio_path=?11, kind=?12 WHERE id=?1",
            params![e.id, e.created_at, e.status.as_str(), e.raw_text, e.final_text, e.app, e.category, e.language,
                    e.duration_ms, e.words, e.audio_path, e.kind.as_str()],
        )?;
        Ok(())
    }

    pub fn get_history(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let sql = format!("SELECT {HISTORY_COLS} FROM history WHERE id=?1");
        Ok(self.conn.query_row(&sql, [id], history_row).optional()?)
    }

    /// Newest first; `query` searches raw and final text.
    pub fn list_history(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<HistoryEntry>> {
        let sql = format!(
            "SELECT {HISTORY_COLS} FROM history WHERE (?1 = '' OR raw_text LIKE '%' || ?1 || '%' OR final_text LIKE '%' || ?1 || '%')
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(params![query, limit, offset], history_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_history(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM history WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn last_done(&self) -> Result<Option<HistoryEntry>> {
        let sql = format!("SELECT {HISTORY_COLS} FROM history WHERE status='Done' ORDER BY created_at DESC, id DESC LIMIT 1");
        Ok(self.conn.query_row(&sql, [], history_row).optional()?)
    }

    /// Rows left open by a crash become `Interrupted` so the user can retry them.
    pub fn mark_interrupted(&self) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE history SET status='Interrupted' WHERE status IN ('Recording', 'Processing')",
            [],
        )?)
    }

    pub fn expired_audio(&self, before_ms: i64) -> Result<Vec<(i64, String)>> {
        let mut st = self.conn.prepare(
            "SELECT id, audio_path FROM history WHERE audio_path IS NOT NULL AND created_at < ?1 ORDER BY id",
        )?;
        let rows = st.query_map([before_ms], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_audio(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE history SET audio_path=NULL WHERE id=?1", [id])?;
        Ok(())
    }

    // ---- dictionary

    pub fn add_dict(&self, term: &str, replacement: Option<&str>) -> Result<i64> {
        let term = term.trim();
        too_long(term, MAX_TERM)?;
        let lower = term.to_lowercase();
        if self.list_dict()?.iter().any(|d| d.term.to_lowercase() == lower) {
            return Err(DbError::Duplicate);
        }
        self.conn.execute("INSERT INTO dictionary (term, replacement) VALUES (?1, ?2)", params![term, replacement])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_dict(&self) -> Result<Vec<DictEntry>> {
        let mut st = self.conn.prepare("SELECT id, term, replacement FROM dictionary ORDER BY id")?;
        let rows = st.query_map([], |r| Ok(DictEntry { id: r.get(0)?, term: r.get(1)?, replacement: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_dict(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM dictionary WHERE id=?1", [id])?;
        Ok(())
    }

    // ---- snippets

    fn check_snippet(&self, id: i64, cue: &str, text: &str) -> Result<()> {
        too_long(cue, MAX_TERM)?;
        too_long(text, MAX_SNIPPET_TEXT)?;
        let lower = cue.to_lowercase();
        if self.list_snippets()?.iter().any(|s| s.id != id && s.cue.to_lowercase() == lower) {
            return Err(DbError::Duplicate);
        }
        Ok(())
    }

    pub fn add_snippet(&self, cue: &str, text: &str) -> Result<i64> {
        let cue = cue.trim();
        self.check_snippet(0, cue, text)?;
        self.conn.execute("INSERT INTO snippets (cue, text) VALUES (?1, ?2)", params![cue, text])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_snippet(&self, s: &Snippet) -> Result<()> {
        let cue = s.cue.trim();
        self.check_snippet(s.id, cue, &s.text)?;
        self.conn.execute("UPDATE snippets SET cue=?2, text=?3 WHERE id=?1", params![s.id, cue, s.text])?;
        Ok(())
    }

    pub fn list_snippets(&self) -> Result<Vec<Snippet>> {
        let mut st = self.conn.prepare("SELECT id, cue, text FROM snippets ORDER BY id")?;
        let rows = st.query_map([], |r| Ok(Snippet { id: r.get(0)?, cue: r.get(1)?, text: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_snippet(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM snippets WHERE id=?1", [id])?;
        Ok(())
    }

    // ---- transforms

    pub fn list_transforms(&self) -> Result<Vec<Transform>> {
        let mut st = self.conn.prepare("SELECT id, name, prompt, hotkey, samples, builtin FROM transforms ORDER BY id")?;
        let rows = st.query_map([], |r| {
            let samples: String = r.get(4)?;
            Ok(Transform {
                id: r.get(0)?,
                name: r.get(1)?,
                prompt: r.get(2)?,
                hotkey: r.get(3)?,
                samples: serde_json::from_str(&samples).unwrap_or_default(),
                builtin: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Inserts when `id == 0`, otherwise updates. `builtin` is never changed by callers.
    pub fn save_transform(&self, t: &Transform) -> Result<i64> {
        let samples = serde_json::to_string(&t.samples).expect("strings serialize");
        if t.id == 0 {
            let custom: i64 = self.conn.query_row("SELECT COUNT(*) FROM transforms WHERE builtin=0", [], |r| r.get(0))?;
            if custom >= MAX_CUSTOM_TRANSFORMS {
                return Err(DbError::Limit);
            }
            self.conn.execute(
                "INSERT INTO transforms (name, prompt, hotkey, samples, builtin) VALUES (?1, ?2, ?3, ?4, 0)",
                params![t.name, t.prompt, t.hotkey, samples],
            )?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn.execute(
                "UPDATE transforms SET name=?2, prompt=?3, hotkey=?4, samples=?5 WHERE id=?1",
                params![t.id, t.name, t.prompt, t.hotkey, samples],
            )?;
            Ok(t.id)
        }
    }

    pub fn delete_transform(&self, id: i64) -> Result<()> {
        let builtin: bool = self.conn.query_row("SELECT builtin FROM transforms WHERE id=?1", [id], |r| r.get(0))?;
        if builtin {
            return Err(DbError::Builtin);
        }
        self.conn.execute("DELETE FROM transforms WHERE id=?1", [id])?;
        Ok(())
    }

    // ---- notes

    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let mut st = self.conn.prepare("SELECT id, title, body, updated_at FROM notes ORDER BY updated_at DESC")?;
        let rows = st.query_map([], |r| Ok(Note { id: r.get(0)?, title: r.get(1)?, body: r.get(2)?, updated_at: r.get(3)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Inserts when `id == 0`, otherwise updates. Returns the id.
    pub fn save_note(&self, n: &Note) -> Result<i64> {
        if n.id == 0 {
            self.conn.execute(
                "INSERT INTO notes (title, body, updated_at) VALUES (?1, ?2, ?3)",
                params![n.title, n.body, n.updated_at],
            )?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn.execute(
                "UPDATE notes SET title=?2, body=?3, updated_at=?4 WHERE id=?1",
                params![n.id, n.title, n.body, n.updated_at],
            )?;
            Ok(n.id)
        }
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM notes WHERE id=?1", [id])?;
        Ok(())
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn entry(created_at: i64, raw: &str, final_text: &str) -> HistoryEntry {
        HistoryEntry {
            id: 0,
            created_at,
            status: Status::Done,
            raw_text: raw.into(),
            final_text: final_text.into(),
            app: "slack.exe".into(),
            category: "Work".into(),
            language: "en".into(),
            duration_ms: 6000,
            words: final_text.split_whitespace().count() as i64,
            audio_path: None,
            kind: Kind::Dictation,
        }
    }

    #[test]
    fn history_crud_and_search() {
        let db = Db::open_in_memory().unwrap();
        let a = db.insert_history(&entry(1, "um hello there", "Hello there.")).unwrap();
        let b = db.insert_history(&entry(2, "launch slips", "The launch slips to Monday.")).unwrap();
        let all = db.list_history("", 50, 0).unwrap();
        assert_eq!(all.iter().map(|e| e.id).collect::<Vec<_>>(), vec![b, a]);
        assert_eq!(db.list_history("monday", 50, 0).unwrap()[0].id, b);
        assert_eq!(db.list_history("um hello", 50, 0).unwrap()[0].id, a);
        let mut e = all[0].clone();
        e.final_text = "Changed.".into();
        db.update_history(&e).unwrap();
        assert_eq!(db.list_history("changed", 50, 0).unwrap().len(), 1);
        assert_eq!(db.last_done().unwrap().unwrap().id, b);
        db.delete_history(b).unwrap();
        assert_eq!(db.list_history("", 50, 0).unwrap().len(), 1);
    }

    #[test]
    fn mark_interrupted_only_touches_open_rows() {
        let db = Db::open_in_memory().unwrap();
        let mut e = entry(1, "", "");
        e.status = Status::Recording;
        db.insert_history(&e).unwrap();
        e.status = Status::Processing;
        db.insert_history(&e).unwrap();
        db.insert_history(&entry(2, "x", "x")).unwrap();
        assert_eq!(db.mark_interrupted().unwrap(), 2);
        let st: Vec<Status> = db.list_history("", 50, 0).unwrap().iter().map(|e| e.status).collect();
        assert_eq!(st.iter().filter(|s| **s == Status::Interrupted).count(), 2);
        assert_eq!(st.iter().filter(|s| **s == Status::Done).count(), 1);
    }

    #[test]
    fn expired_audio_listed_and_cleared() {
        let db = Db::open_in_memory().unwrap();
        let mut e = entry(1000, "a", "a");
        e.audio_path = Some("audio/1000.wav".into());
        let id = db.insert_history(&e).unwrap();
        e.created_at = 5000;
        e.audio_path = Some("audio/5000.wav".into());
        db.insert_history(&e).unwrap();
        assert_eq!(db.expired_audio(2000).unwrap(), vec![(id, "audio/1000.wav".to_string())]);
        db.clear_audio(id).unwrap();
        assert!(db.expired_audio(2000).unwrap().is_empty());
    }

    #[test]
    fn dict_rejects_dupes_case_insensitive() {
        let db = Db::open_in_memory().unwrap();
        db.add_dict("Grüße", None).unwrap();
        assert!(matches!(db.add_dict("grüße", None), Err(DbError::Duplicate)));
        assert!(db.add_dict("GRÜSSE", None).is_ok());
        assert!(matches!(db.add_dict(&"x".repeat(61), None), Err(DbError::TooLong)));
        db.add_dict("gonna", Some("going to")).unwrap();
        let d = db.list_dict().unwrap();
        assert_eq!(d.len(), 3);
        assert_eq!(d.iter().find(|e| e.term == "gonna").unwrap().replacement.as_deref(), Some("going to"));
        db.delete_dict(d[0].id).unwrap();
        assert_eq!(db.list_dict().unwrap().len(), 2);
    }

    #[test]
    fn snippet_limits() {
        let db = Db::open_in_memory().unwrap();
        let id = db.add_snippet("my calendar", "https://cal.com/me").unwrap();
        assert!(matches!(db.add_snippet("My Calendar", "x"), Err(DbError::Duplicate)));
        assert!(matches!(db.add_snippet(&"c".repeat(61), "x"), Err(DbError::TooLong)));
        assert!(matches!(db.add_snippet("long", &"t".repeat(4001)), Err(DbError::TooLong)));
        db.update_snippet(&Snippet { id, cue: "my cal".into(), text: "new".into() }).unwrap();
        assert_eq!(db.list_snippets().unwrap()[0].text, "new");
        db.delete_snippet(id).unwrap();
        assert!(db.list_snippets().unwrap().is_empty());
    }

    #[test]
    fn transform_limit_8_and_builtin_undeletable() {
        let db = Db::open_in_memory().unwrap();
        let ts = db.list_transforms().unwrap();
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].name, "Polish");
        assert!(ts[0].builtin);
        assert_eq!(ts[0].hotkey.as_deref(), Some("Win+Alt+1"));
        assert!(matches!(db.delete_transform(ts[0].id), Err(DbError::Builtin)));
        for i in 0..8 {
            let t = Transform { id: 0, name: format!("t{i}"), prompt: "p".into(), hotkey: None, samples: vec!["s".into()], builtin: false };
            db.save_transform(&t).unwrap();
        }
        let t = Transform { id: 0, name: "t9".into(), prompt: "p".into(), hotkey: None, samples: vec![], builtin: false };
        assert!(matches!(db.save_transform(&t), Err(DbError::Limit)));
        let mut first = db.list_transforms().unwrap()[1].clone();
        assert_eq!(first.samples, vec!["s".to_string()]);
        first.name = "renamed".into();
        db.save_transform(&first).unwrap();
        assert_eq!(db.list_transforms().unwrap()[1].name, "renamed");
    }

    #[test]
    fn notes_crud() {
        let db = Db::open_in_memory().unwrap();
        let id = db.save_note(&Note { id: 0, title: "a".into(), body: "b".into(), updated_at: 1 }).unwrap();
        db.save_note(&Note { id, title: "a2".into(), body: "b2".into(), updated_at: 2 }).unwrap();
        let n = db.list_notes().unwrap();
        assert_eq!((n.len(), n[0].title.as_str()), (1, "a2"));
        db.delete_note(id).unwrap();
        assert!(db.list_notes().unwrap().is_empty());
    }
}
