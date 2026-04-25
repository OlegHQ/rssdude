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

/// Compute a since-cutoff datetime from a duration string like "24h", "7d".
pub fn since_cutoff(since: &str) -> anyhow::Result<NaiveDateTime> {
    Ok(Utc::now().naive_utc() - parse_duration(since)?)
}

/// Build standard item table rows from raw JSON values: [ID, SOURCE, TITLE, PUBLISHED].
pub fn item_value_rows(items: &[serde_json::Value]) -> Vec<Vec<String>> {
    items.iter().map(|i| vec![
        i["id"].as_str().unwrap_or("").to_string(),
        i["source"].as_str().unwrap_or("-").to_string(),
        i["title"].as_str().unwrap_or("-").to_string(),
        i["published_at"].as_str().map(time_ago).unwrap_or_else(|| "-".into()),
    ]).collect()
}

/// Build standard item table rows: [ID, SOURCE, TITLE, PUBLISHED].
pub fn item_table_rows(items: &[super::db::ItemJson]) -> Vec<Vec<String>> {
    items.iter().map(|ij| vec![
        ij.id.clone(),
        ij.source.clone().unwrap_or("-".into()),
        ij.title.clone().unwrap_or("-".into()),
        ij.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
    ]).collect()
}

/// Format an item for export in md/json/text format.
pub fn format_export(item: &super::db::ItemJson, format: &str) -> anyhow::Result<String> {
    let title = item.title.as_deref().unwrap_or("(untitled)");
    let source = item.source.as_deref().unwrap_or("-");
    let published = item.published_at.as_deref().unwrap_or("-");
    let url = item.link.as_deref().unwrap_or("-");
    let note = item.note.as_deref().unwrap_or("");
    let content = item.content.as_deref().or(item.summary.as_deref()).unwrap_or("");

    match format {
        "md" | "markdown" => {
            let mut s = format!("# {title}\n\n**Source:** {source}\n**Published:** {published}\n**URL:** {url}\n");
            if !note.is_empty() { s.push_str(&format!("**Note:** {note}\n")); }
            s.push_str(&format!("\n---\n\n{content}"));
            Ok(s)
        }
        "json" => Ok(serde_json::to_string_pretty(item).unwrap_or_else(|_| "{}".to_string())),
        "text" | "txt" => {
            let mut s = format!("{title}\nSource: {source}\nPublished: {published}\nURL: {url}\n");
            if !note.is_empty() { s.push_str(&format!("Note: {note}\n")); }
            s.push_str(&format!("\n{content}"));
            Ok(s)
        }
        _ => anyhow::bail!("unsupported format: {format} (expected md, json, or text)"),
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
