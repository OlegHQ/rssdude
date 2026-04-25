use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

#[derive(Serialize)]
pub struct Digest {
    pub since: String,
    pub total: usize,
    pub feeds: Vec<DigestFeed>,
}

#[derive(Serialize)]
pub struct DigestFeed {
    pub feed_id: String,
    pub feed_title: String,
    pub items: Vec<DigestItem>,
}

#[derive(Serialize)]
pub struct DigestItem {
    pub id: String,
    pub title: String,
}

/// Core: produce digest data.
pub async fn digest_core(db: Arc<Database<'static>>, since: String, tag: Option<String>) -> Result<Digest> {
    let cutoff = since_cutoff(&since)?;

    spawn_blocking(move || -> Result<Digest> {
        let r = db.r_transaction().context("read transaction")?;
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let valid_feed_ids: Option<HashSet<String>> = if let Some(ref t) = tag {
            let all_feeds: Vec<Feed> = r.scan().primary::<Feed>()?.all()?.filter_map(|f| f.ok()).collect();
            Some(all_feeds.iter()
                .filter(|f| f.has_tag(t))
                .map(|f| f.id.clone()).collect())
        } else { None };

        let mut grouped: HashMap<String, Vec<Item>> = HashMap::new();
        for item in all_items {
            if let Some(ref valid) = valid_feed_ids { if !valid.contains(&item.feed_id) { continue; } }
            if let Some(ref pub_at) = item.published_at {
                if let Ok(dt) = parse_datetime(pub_at) {
                    if dt >= cutoff { grouped.entry(item.feed_id.clone()).or_default().push(item); }
                }
            }
        }

        let mut feeds = Vec::new();
        let mut total = 0;
        let mut feed_ids: Vec<String> = grouped.keys().cloned().collect();
        feed_ids.sort();
        for fid in feed_ids {
            let items = grouped.remove(&fid).unwrap_or_default();
            let feed: Option<Feed> = r.get().primary(fid.clone()).ok().flatten();
            let feed_title = feed.as_ref().map(|f| f.display_title().to_string()).unwrap_or_else(|| fid.clone());
            total += items.len();
            let digest_items = items.iter().map(|i| DigestItem {
                id: i.id.clone(), title: i.title.clone().unwrap_or("(untitled)".into()),
            }).collect();
            feeds.push(DigestFeed { feed_id: fid, feed_title, items: digest_items });
        }
        Ok(Digest { since: cutoff.format("%Y-%m-%d %H:%M").to_string(), total, feeds })
    }).await?
}

pub async fn digest(db: Arc<Database<'static>>, json: bool, since: String, tag: Option<String>) -> Result<()> {
    let d = digest_core(db, since, tag).await?;
    if json {
        print_json(&d);
    } else {
        println!("Digest: {} new items since {}", d.total, d.since);
        println!();
        for feed in &d.feeds {
            println!("{} ({} new)", feed.feed_title, feed.items.len());
            for item in &feed.items { println!("  - {}", item.title); }
            println!();
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct TrendingTopic {
    pub topic: String,
    pub mentions: usize,
    pub feeds: usize,
    pub latest: String,
}

/// Core: compute trending topics.
pub async fn trending_core(db: Arc<Database<'static>>) -> Result<Vec<TrendingTopic>> {
    let cutoff = since_cutoff("48h")?;
    let sw = stop_words::get(stop_words::LANGUAGE::English);
    let stopwords: HashSet<String> = sw.into_iter().collect();

    spawn_blocking(move || -> Result<Vec<TrendingTopic>> {
        let r = db.r_transaction().context("read transaction")?;
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut word_stats: HashMap<String, (usize, HashSet<String>, String)> = HashMap::new();

        for item in &all_items {
            let in_window = item.published_at.as_ref()
                .and_then(|p| parse_datetime(p).ok()).is_some_and(|dt| dt >= cutoff);
            if !in_window { continue; }
            if let Some(ref title) = item.title {
                let pub_at = item.published_at.as_deref().unwrap_or("").to_string();
                for word in title.split_whitespace() {
                    let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                    if w.len() < 2 || stopwords.contains(&w) { continue; }
                    let entry = word_stats.entry(w).or_insert_with(|| (0, HashSet::new(), String::new()));
                    entry.0 += 1;
                    entry.1.insert(item.feed_id.clone());
                    if pub_at > entry.2 { entry.2 = pub_at.clone(); }
                }
            }
        }

        let mut topics: Vec<TrendingTopic> = word_stats.into_iter()
            .filter(|(_, (_, feeds, _))| feeds.len() >= 2)
            .map(|(word, (count, feeds, latest))| TrendingTopic {
                topic: word, mentions: count, feeds: feeds.len(), latest: time_ago(&latest),
            }).collect();
        topics.sort_by_key(|t| std::cmp::Reverse(t.mentions));
        Ok(topics)
    }).await?
}

pub async fn trending(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let topics = trending_core(db).await?;
    if json {
        print_json(&topics);
    } else {
        let rows: Vec<Vec<String>> = topics.iter().map(|t| {
            vec![t.topic.clone(), t.mentions.to_string(), t.feeds.to_string(), t.latest.clone()]
        }).collect();
        print_table(&["TOPIC", "MENTIONS", "FEEDS", "LATEST"], &rows);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct MatchResult {
    #[serde(flatten)]
    pub item: ItemJson,
    pub matched_keywords: String,
}

/// Core: match keywords against recent items.
pub async fn match_keywords_core(
    db: Arc<Database<'static>>,
    keywords: String,
    limit: usize,
    since: String,
) -> Result<Vec<MatchResult>> {
    let cutoff = since_cutoff(&since)?;
    let kw_list: Vec<String> = keywords.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();

    spawn_blocking(move || -> Result<Vec<MatchResult>> {
        let r = db.r_transaction().context("read transaction")?;
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut matched = Vec::new();

        for item in all_items.into_iter().rev() {
            if matched.len() >= limit { break; }
            let in_window = item.published_at.as_ref()
                .and_then(|p| parse_datetime(p).ok()).is_some_and(|dt| dt >= cutoff);
            if !in_window { continue; }

            let title_lower = item.title.as_ref().map(|t| t.to_lowercase()).unwrap_or_default();
            let summary_lower = item.summary.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
            let matched_kws: Vec<&str> = kw_list.iter()
                .filter(|kw| title_lower.contains(kw.as_str()) || summary_lower.contains(kw.as_str()))
                .map(|s| s.as_str()).collect();
            if matched_kws.is_empty() { continue; }

            let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
            matched.push(MatchResult {
                item: ItemJson::from_parts(&item, feed.as_ref(), None),
                matched_keywords: matched_kws.join(", "),
            });
        }
        Ok(matched)
    }).await?
}

pub async fn match_keywords(db: Arc<Database<'static>>, json: bool, keywords: String, limit: usize, since: String) -> Result<()> {
    let results = match_keywords_core(db, keywords, limit, since).await?;
    if json {
        print_json(&results);
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|r| {
            vec![
                r.item.id.clone(),
                r.item.source.clone().unwrap_or("-".into()),
                r.item.title.clone().unwrap_or("-".into()),
                r.matched_keywords.clone(),
                r.item.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
            ]
        }).collect();
        print_table(&["ID", "SOURCE", "TITLE", "MATCHED", "PUBLISHED"], &rows);
    }
    Ok(())
}
