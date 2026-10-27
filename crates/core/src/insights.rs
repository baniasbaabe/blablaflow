use serde::Serialize;

use crate::db::{Db, Result};

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Insights {
    pub total_words: i64,
    pub avg_wpm: i64,
    pub words_this_week: i64,
    pub streak_days: i64,
    pub top_apps: Vec<(String, i64)>,
    /// (YYYY-MM-DD local date, words), oldest first.
    pub daily: Vec<(String, i64)>,
}

/// Stats over completed dictations. `utc_offset_s` defines the user's local day.
pub fn compute(db: &Db, now_ms: i64, utc_offset_s: i64) -> Result<Insights> {
    let c = db.conn();
    let done = "status='Done' AND kind='Dictation'";
    let (total_words, total_ms): (i64, i64) = c.query_row(
        &format!("SELECT COALESCE(SUM(words),0), COALESCE(SUM(duration_ms),0) FROM history WHERE {done}"),
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let words_this_week: i64 = c.query_row(
        &format!("SELECT COALESCE(SUM(words),0) FROM history WHERE {done} AND created_at > ?1"),
        [now_ms - 7 * DAY_MS],
        |r| r.get(0),
    )?;
    let mut st = c.prepare(&format!(
        "SELECT app, SUM(words) w FROM history WHERE {done} GROUP BY app ORDER BY w DESC LIMIT 5"
    ))?;
    let top_apps = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;

    let offset_ms = utc_offset_s * 1000;
    let mut st = c.prepare(&format!(
        "SELECT (created_at + ?1) / {DAY_MS} d, SUM(words) FROM history WHERE {done} GROUP BY d ORDER BY d"
    ))?;
    let days: Vec<(i64, i64)> = st.query_map([offset_ms], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;

    // Streak: consecutive days ending today, or yesterday if nothing yet today.
    let today = (now_ms + offset_ms).div_euclid(DAY_MS);
    let mut day = if days.last().is_some_and(|d| d.0 == today) { today } else { today - 1 };
    let mut streak_days = 0;
    for (d, _) in days.iter().rev() {
        if *d != day {
            break;
        }
        streak_days += 1;
        day -= 1;
    }

    Ok(Insights {
        total_words,
        avg_wpm: if total_ms > 0 { total_words * 60_000 / total_ms } else { 0 },
        words_this_week,
        streak_days,
        top_apps,
        daily: days.into_iter().map(|(d, w)| (civil_date(d), w)).collect(),
    })
}

/// Days since 1970-01-01 → "YYYY-MM-DD" (Howard Hinnant's civil_from_days).
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Db, HistoryEntry, Kind, Status};

    const DAY: i64 = 86_400_000;

    fn add(db: &Db, at: i64, words: i64, app: &str) {
        db.insert_history(&HistoryEntry {
            id: 0, created_at: at, status: Status::Done, raw_text: String::new(), final_text: String::new(),
            app: app.into(), category: "Other".into(), language: "en".into(), duration_ms: 30_000, words,
            audio_path: None, kind: Kind::Dictation,
        }).unwrap();
    }

    #[test]
    fn insights_streak_and_wpm() {
        let db = Db::open_in_memory().unwrap();
        let now = 100 * DAY + 12 * 3_600_000; // day 100, noon UTC
        add(&db, now - 9 * DAY, 10, "notepad.exe"); // gap before the streak
        add(&db, now - 2 * DAY, 50, "slack.exe");
        add(&db, now - DAY, 50, "slack.exe");
        add(&db, now, 40, "code.exe");
        let i = compute(&db, now, 0).unwrap();
        assert_eq!(i.total_words, 150);
        assert_eq!(i.avg_wpm, 75); // 150 words over 2 minutes
        assert_eq!(i.streak_days, 3);
        assert_eq!(i.words_this_week, 140);
        assert_eq!(i.top_apps[0], ("slack.exe".to_string(), 100));
        assert_eq!(i.daily.len(), 4);
        assert_eq!(i.daily.last().unwrap().0, "1970-04-11");
    }

    #[test]
    fn streak_counts_from_yesterday_and_respects_offset() {
        let db = Db::open_in_memory().unwrap();
        let now = 100 * DAY + 3_600_000; // 01:00 UTC
        add(&db, now - DAY, 5, "a"); // yesterday 01:00 UTC
        assert_eq!(compute(&db, now, 0).unwrap().streak_days, 1);
        // At UTC-2 "now" is still day 99 and the entry is day 98: streak 1 ending yesterday.
        assert_eq!(compute(&db, now, -7200).unwrap().streak_days, 1);
        let empty = Db::open_in_memory().unwrap();
        let i = compute(&empty, now, 0).unwrap();
        assert_eq!((i.total_words, i.avg_wpm, i.streak_days), (0, 0, 0));
    }

    #[test]
    fn non_dictation_and_failed_rows_ignored() {
        let db = Db::open_in_memory().unwrap();
        add(&db, 0, 10, "a");
        let mut e = db.list_history("", 1, 0).unwrap()[0].clone();
        e.kind = Kind::Transform;
        db.insert_history(&e).unwrap();
        e.kind = Kind::Dictation;
        e.status = Status::Failed;
        db.insert_history(&e).unwrap();
        assert_eq!(compute(&db, 0, 0).unwrap().total_words, 10);
    }
}
