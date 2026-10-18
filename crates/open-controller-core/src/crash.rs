//! What a crash leaves behind. Release builds abort on a panic and, on Windows, have no console,
//! so without this a crash would leave no trace at all. The panic is added to `crash.log` in the
//! data folder, which keeps only its last few entries, before the previous hook runs.

use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOG_FILE: &str = "crash.log";
/// The log's size cap. Older entries are dropped, whole, to stay under it.
const KEEP: usize = 64 * 1024;

/// Logs every panic to `data_dir/crash.log`, then calls `on_crash` (for what has to be undone
/// before the process ends) and the hook that was there before.
pub fn install(data_dir: &Path, on_crash: impl Fn() + Send + Sync + 'static) {
    let path = data_dir.join(LOG_FILE);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = append(&path, &entry(info, SystemTime::now()));
        on_crash();
        previous(info);
    }));
}

/// Where the resident keeps its settings, for a program that has no `--data-dir` of its own to
/// go by (the window).
pub fn data_dir() -> PathBuf {
    const APP: &str = "io.github.brunovncs.open-controller";
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join(APP)
}

fn entry(info: &PanicHookInfo, now: SystemTime) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "open-controller".into());
    let thread = std::thread::current();
    let message = info.payload_as_str().unwrap_or("(no message)");
    let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
    format!(
        "{} {program} {}: thread '{}' panicked at {place}:\n{message}\n\n",
        timestamp(now),
        env!("CARGO_PKG_VERSION"),
        thread.name().unwrap_or("<unnamed>")
    )
}

/// UTC, as `2026-10-18 14:30:05Z`, without a date library.
fn timestamp(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z", rest / 3_600, rest % 3_600 / 60, rest % 60)
}

fn append(path: &Path, entry: &str) -> std::io::Result<()> {
    let mut log = std::fs::read(path).unwrap_or_default();
    log.extend_from_slice(entry.as_bytes());
    std::fs::write(path, trimmed(&log))
}

/// The last `KEEP` bytes, from the start of an entry (entries end in a blank line).
fn trimmed(log: &[u8]) -> &[u8] {
    if log.len() <= KEEP {
        return log;
    }
    let tail = &log[log.len() - KEEP..];
    match tail.windows(2).position(|w| w == b"\n\n") {
        Some(i) => &tail[i + 2..],
        None => tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timestamps_are_utc_dates() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01 00:00:00Z");
        assert_eq!(timestamp(UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29 00:00:00Z");
        assert_eq!(timestamp(UNIX_EPOCH + Duration::from_secs(1_792_333_805)), "2026-10-18 14:30:05Z");
    }

    #[test]
    fn the_log_keeps_its_last_whole_entries() {
        let path = std::env::temp_dir().join(format!("oc-crash-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let entry = |i: usize| format!("entry {i}\n{}\n\n", "x".repeat(1_000));
        for i in 0..200 {
            append(&path, &entry(i)).unwrap();
        }
        let log = std::fs::read_to_string(&path).unwrap();
        assert!(log.len() <= KEEP);
        assert!(log.starts_with("entry "), "starts on an entry");
        assert!(log.ends_with(&entry(199)));
        assert!(!log.contains(&entry(0)));
        let _ = std::fs::remove_file(&path);
    }
}
