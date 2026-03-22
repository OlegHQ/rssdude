use anyhow::{Context, Result};
use feed_rs::parser;
use reqwest::header;

use crate::db::{gen_id, Item};

pub struct FetchResult {
    pub feed: feed_rs::model::Feed,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Convert parsed feed entries into Item structs. Shared helper — use this
/// instead of duplicating the entry→Item conversion loop.
pub fn entries_to_items(entries: &[feed_rs::model::Entry], feed_id: &str, now: &str) -> Vec<Item> {
    entries
        .iter()
        .map(|entry| {
            let guid = if !entry.id.is_empty() {
                entry.id.clone()
            } else {
                entry.links.first().map(|l| l.href.clone()).unwrap_or_else(gen_id)
            };
            Item {
                id: gen_id(),
                feed_id: feed_id.to_string(),
                guid,
                title: entry.title.as_ref().map(|t| t.content.clone()),
                link: entry.links.first().map(|l| l.href.clone()),
                content: entry.content.as_ref().and_then(|c| c.body.clone()),
                summary: entry.summary.as_ref().map(|s| s.content.clone()),
                published_at: entry.published.or(entry.updated).map(|d| d.to_rfc3339()),
                fetched_at: now.to_string(),
            }
        })
        .collect()
}

/// Fetch and parse a feed from URL.
pub async fn fetch_feed(url: &str) -> Result<FetchResult> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header(header::USER_AGENT, "rssdude/0.1")
        .send()
        .await
        .with_context(|| format!("failed to fetch {url}"))?;

    let etag = resp
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let last_modified = resp
        .headers()
        .get(header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("failed to read body from {url}"))?;
    let feed =
        parser::parse(&bytes[..]).with_context(|| format!("failed to parse feed from {url}"))?;

    Ok(FetchResult {
        feed,
        etag,
        last_modified,
    })
}

/// Fetch with conditional headers (If-None-Match, If-Modified-Since).
/// Returns None on 304 Not Modified.
pub async fn fetch_feed_conditional(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<Option<FetchResult>> {
    let client = reqwest::Client::new();
    let mut req = client.get(url).header(header::USER_AGENT, "rssdude/0.1");

    if let Some(etag) = etag {
        req = req.header(header::IF_NONE_MATCH, etag);
    }
    if let Some(lm) = last_modified {
        req = req.header(header::IF_MODIFIED_SINCE, lm);
    }

    let resp = req
        .send()
        .await
        .with_context(|| format!("failed to fetch {url}"))?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }

    let new_etag = resp
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let new_lm = resp
        .headers()
        .get(header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("failed to read body from {url}"))?;
    let feed =
        parser::parse(&bytes[..]).with_context(|| format!("failed to parse feed from {url}"))?;

    Ok(Some(FetchResult {
        feed,
        etag: new_etag,
        last_modified: new_lm,
    }))
}
