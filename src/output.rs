use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use serde::Serialize;

/// Print any Serialize type as pretty JSON.
pub fn print_json<T: Serialize>(data: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string())
    );
}

/// Print aligned table with headers.
///
/// Column widths are calculated from headers and data. Columns are left-aligned
/// with 2-space gaps. Prints "No results." if rows is empty.
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("No results.");
        return;
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Print header
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    println!("{}", header_line.join("  "));

    // Print rows
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let w = widths.get(i).copied().unwrap_or(cell.len());
                format!("{:<width$}", cell, width = w)
            })
            .collect();
        println!("{}", line.join("  "));
    }
}

/// Convert an ISO 8601 datetime string to a human-readable relative time string
/// such as "Xm ago", "Xh ago", or "Xd ago".
///
/// Tries parsing with RFC 3339, `%Y-%m-%dT%H:%M:%S`, and `%Y-%m-%d %H:%M:%S`.
pub fn time_ago(iso: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.naive_utc())
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%d %H:%M:%S"))
    else {
        return iso.to_string();
    };

    let now = Utc::now().naive_utc();
    let duration = now.signed_duration_since(dt);

    if duration.num_minutes() < 1 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else {
        format!("{}d ago", duration.num_days())
    }
}

/// Parse duration strings like "24h", "7d", "1w", "2h30m" into `chrono::Duration`.
pub fn parse_duration(s: &str) -> Result<chrono::Duration> {
    let std_dur = humantime::parse_duration(s.trim())
        .map_err(|e| anyhow::anyhow!("invalid duration {s:?}: {e}"))?;
    chrono::Duration::from_std(std_dur)
        .map_err(|e| anyhow::anyhow!("duration out of range: {e}"))
}

/// Parse datetime strings in multiple formats (RFC 3339, ISO 8601 variants).
/// Shared helper — use this instead of duplicating parse logic.
pub fn parse_datetime(s: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.naive_utc())
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
}
