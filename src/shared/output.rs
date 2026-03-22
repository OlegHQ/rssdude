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
/// Prints "No results." if rows is empty.
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("No results.");
        return;
    }
    use tabled::{builder::Builder, settings::Style};
    let mut builder = Builder::default();
    builder.push_record(headers.iter().map(|s| s.to_string()));
    for row in rows {
        builder.push_record(row.iter().map(|s| s.to_string()));
    }
    println!("{}", builder.build().with(Style::blank()));
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

/// Convert HTML to plain text, wrapped to the given width.
pub fn strip_html(html: &str, width: usize) -> String {
    html2text::from_read(html.as_bytes(), width.max(20)).trim().to_string()
}

/// Print item detail view (Source/Title/Published/URL/body).
pub fn print_item_detail(source: &str, title: &str, published: &str, url: &str, body: &str) {
    println!("Source:    {source}");
    println!("Title:     {title}");
    println!("Published: {published}");
    println!("URL:       {url}");
    println!("---");
    println!("{body}");
}

/// Format an item for export in the given format (md, json, text/txt).
pub fn format_item_export(
    title: &str, source: &str, published: &str, url: &str,
    note: &str, content: &str, format: &str,
) -> String {
    match format {
        "md" | "markdown" => {
            let mut s = format!("# {title}\n\n**Source:** {source}\n**Published:** {published}\n**URL:** {url}\n");
            if !note.is_empty() { s.push_str(&format!("**Note:** {note}\n")); }
            s.push_str(&format!("\n---\n\n{content}"));
            s
        }
        _ => {
            let mut s = format!("{title}\nSource: {source}\nPublished: {published}\nURL: {url}\n");
            if !note.is_empty() { s.push_str(&format!("Note: {note}\n")); }
            s.push_str(&format!("\n{content}"));
            s
        }
    }
}

/// Parse datetime strings in multiple formats (RFC 3339, ISO 8601 variants).
/// Shared helper — use this instead of duplicating parse logic.
pub fn parse_datetime(s: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.naive_utc())
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
}
