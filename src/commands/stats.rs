use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono::Timelike;
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

#[derive(Serialize)]
pub struct StatsResult {
    pub window_days: u32,
    pub activity: ActivityStats,
    pub per_feed: Vec<FeedEngagement>,
    pub by_hour: Vec<HourBucket>,
    pub by_folder: Vec<FolderEngagement>,
    pub dead_feeds: Vec<DeadFeed>,
    pub streak: u32,
}

#[derive(Serialize)]
pub struct ActivityStats {
    pub articles_read: u32,
    pub articles_opened: u32,
    pub articles_starred: u32,
    pub daily_avg: f32,
    pub open_rate: f32,
    pub star_rate: f32,
}

#[derive(Serialize)]
pub struct FeedEngagement {
    pub feed_id: String,
    pub feed_name: String,
    pub read: u32,
    pub opened: u32,
    pub skipped: u32,
    pub engagement_pct: f32,
    pub is_low: bool,
}

#[derive(Serialize)]
pub struct HourBucket {
    pub hour_range: String,
    pub count: u32,
}

#[derive(Serialize)]
pub struct FolderEngagement {
    pub folder_name: String,
    pub depth: usize,
    pub articles: u32,
    pub engagement_pct: f32,
}

#[derive(Serialize)]
pub struct DeadFeed {
    pub feed_id: String,
    pub feed_name: String,
    pub skipped: u32,
    pub engagement_pct: f32,
}

pub async fn stats_core(
    db: Arc<Database<'static>>,
    since: &str,
    dead_threshold: Option<f32>,
) -> Result<StatsResult> {
    let dur = parse_duration(since)?;
    let since_days = dur.num_days().max(1) as u32;
    spawn_blocking(move || {
        let r = db.r_transaction()?;
        let feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let marks: Vec<Mark> = r.scan().primary()?.all()?.filter_map(|m| m.ok()).collect();
        let folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|f| f.ok()).collect();
        drop(r);

        let now = chrono::Utc::now().naive_utc();
        let cutoff = now - dur;
        let threshold = dead_threshold.unwrap_or(5.0);

        let feed_map: HashMap<&str, &Feed> = feeds.iter().map(|f| (f.id.as_str(), f)).collect();
        let mark_map: HashMap<&str, &Mark> = marks.iter().map(|m| (m.item_id.as_str(), m)).collect();

        // Window engagement (per-feed, per-folder, dead-feed) by item.published_at:
        // "did I read the things published this month".
        let window_items: Vec<&Item> = items.iter().filter(|item| {
            item.published_at.as_ref()
                .and_then(|p| parse_datetime(p).ok())
                .is_some_and(|dt| dt >= cutoff)
        }).collect();
        let counters = compute_counters(&window_items, &mark_map);

        // Reading activity (top-line counts, hour buckets) by mark.read_at:
        // "what I actually read this month, regardless of when it was published".
        let activity_counters = compute_activity_counters(&items, &marks, cutoff);
        let activity = build_activity(&activity_counters, since_days);
        let by_hour = build_hour_buckets(&activity_counters.hour_counts);

        let (per_feed, dead_feeds) = build_feed_engagement(&counters, &feed_map, threshold);
        let by_folder = build_folder_engagement(&folders, &feeds, &counters, &feed_map);
        let streak = compute_streak(&marks, now);

        Ok(StatsResult {
            window_days: since_days, activity, per_feed, by_hour, by_folder, dead_feeds, streak,
        })
    }).await?
}

struct Counters {
    total_read: u32,
    total_opened: u32,
    total_starred: u32,
    feed_read: HashMap<String, u32>,
    feed_opened: HashMap<String, u32>,
    feed_skipped: HashMap<String, u32>,
    hour_counts: [u32; 8],
}

fn compute_counters(items: &[&Item], mark_map: &HashMap<&str, &Mark>) -> Counters {
    let mut c = Counters {
        total_read: 0, total_opened: 0, total_starred: 0,
        feed_read: HashMap::new(), feed_opened: HashMap::new(), feed_skipped: HashMap::new(),
        hour_counts: [0; 8],
    };
    for item in items {
        let mark = mark_map.get(item.id.as_str());
        let is_read = mark.is_some_and(|m| m.read);
        if is_read {
            c.total_read += 1;
            *c.feed_read.entry(item.feed_id.clone()).or_default() += 1;
            if let Some(read_at) = mark.and_then(|m| m.read_at.as_ref()) {
                if let Ok(dt) = parse_datetime(read_at) {
                    c.hour_counts[dt.time().hour() as usize / 3] += 1;
                }
            }
        } else {
            *c.feed_skipped.entry(item.feed_id.clone()).or_default() += 1;
        }
        if mark.is_some_and(|m| m.opened_at.is_some()) {
            c.total_opened += 1;
            *c.feed_opened.entry(item.feed_id.clone()).or_default() += 1;
        }
        if mark.is_some_and(|m| m.starred) { c.total_starred += 1; }
    }
    c
}

/// Activity counters keyed off mark.read_at within the window — these answer
/// "what did I actually read this month", which is independent of when the
/// items were published.
fn compute_activity_counters(items: &[Item], marks: &[Mark], cutoff: chrono::NaiveDateTime) -> Counters {
    let item_map: HashMap<&str, &Item> = items.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut c = Counters {
        total_read: 0, total_opened: 0, total_starred: 0,
        feed_read: HashMap::new(), feed_opened: HashMap::new(), feed_skipped: HashMap::new(),
        hour_counts: [0; 8],
    };
    for mark in marks {
        let Some(read_at) = mark.read_at.as_ref().and_then(|s| parse_datetime(s).ok())
        else { continue };
        if read_at < cutoff { continue; }
        if !mark.read { continue; }
        c.total_read += 1;
        c.hour_counts[read_at.time().hour() as usize / 3] += 1;
        if mark.starred { c.total_starred += 1; }
        if let Some(item) = item_map.get(mark.item_id.as_str()) {
            *c.feed_read.entry(item.feed_id.clone()).or_default() += 1;
            if mark.opened_at.is_some() {
                c.total_opened += 1;
                *c.feed_opened.entry(item.feed_id.clone()).or_default() += 1;
            }
        }
    }
    c
}

fn build_activity(c: &Counters, days: u32) -> ActivityStats {
    let daily_avg = if days > 0 { c.total_read as f32 / days as f32 } else { 0.0 };
    let open_rate = if c.total_read > 0 { c.total_opened as f32 / c.total_read as f32 * 100.0 } else { 0.0 };
    let star_rate = if c.total_read > 0 { c.total_starred as f32 / c.total_read as f32 * 100.0 } else { 0.0 };
    ActivityStats {
        articles_read: c.total_read, articles_opened: c.total_opened,
        articles_starred: c.total_starred, daily_avg, open_rate, star_rate,
    }
}

fn build_feed_engagement(
    c: &Counters, feed_map: &HashMap<&str, &Feed>, threshold: f32,
) -> (Vec<FeedEngagement>, Vec<DeadFeed>) {
    let all_ids: HashSet<&str> = c.feed_read.keys().chain(c.feed_skipped.keys()).map(|s| s.as_str()).collect();
    let mut per_feed = Vec::new();
    let mut dead = Vec::new();
    for fid in &all_ids {
        let read = c.feed_read.get(*fid).copied().unwrap_or(0);
        let opened = c.feed_opened.get(*fid).copied().unwrap_or(0);
        let skipped = c.feed_skipped.get(*fid).copied().unwrap_or(0);
        let total = read + skipped;
        let pct = if total > 0 { read as f32 / total as f32 * 100.0 } else { 0.0 };
        let name = feed_map.get(fid).map(|f| f.display_title().to_string()).unwrap_or_else(|| fid.to_string());
        per_feed.push(FeedEngagement {
            feed_id: fid.to_string(), feed_name: name.clone(),
            read, opened, skipped, engagement_pct: pct, is_low: pct < 10.0,
        });
        if pct < threshold {
            dead.push(DeadFeed { feed_id: fid.to_string(), feed_name: name, skipped, engagement_pct: pct });
        }
    }
    per_feed.sort_by(|a, b| b.engagement_pct.partial_cmp(&a.engagement_pct).unwrap_or(std::cmp::Ordering::Equal));
    dead.sort_by_key(|d| std::cmp::Reverse(d.skipped));
    (per_feed, dead)
}

fn build_hour_buckets(counts: &[u32; 8]) -> Vec<HourBucket> {
    ["0-3","3-6","6-9","9-12","12-15","15-18","18-21","21-24"].iter().enumerate()
        .map(|(i, label)| HourBucket { hour_range: label.to_string(), count: counts[i] })
        .collect()
}

fn build_folder_engagement(
    folders: &[Folder], feeds: &[Feed], c: &Counters, _feed_map: &HashMap<&str, &Feed>,
) -> Vec<FolderEngagement> {
    let folder_map: HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    folders.iter().map(|folder| {
        let desc_ids = collect_descendant_ids(folders, &folder.id);
        let folder_feed_ids: Vec<&str> = feeds.iter()
            .filter(|f| f.folder_id.as_ref().is_some_and(|fid| desc_ids.contains(fid)))
            .map(|f| f.id.as_str()).collect();
        let read: u32 = folder_feed_ids.iter().map(|fid| c.feed_read.get(*fid).copied().unwrap_or(0)).sum();
        let skip: u32 = folder_feed_ids.iter().map(|fid| c.feed_skipped.get(*fid).copied().unwrap_or(0)).sum();
        let total = read + skip;
        let pct = if total > 0 { read as f32 / total as f32 * 100.0 } else { 0.0 };
        let depth = {
            let mut d = 0usize;
            let mut cur = folder.parent_id.as_deref();
            while let Some(pid) = cur {
                d += 1;
                cur = folder_map.get(pid).and_then(|f| f.parent_id.as_deref());
            }
            d
        };
        FolderEngagement { folder_name: folder.name.clone(), depth, articles: total, engagement_pct: pct }
    }).collect()
}

fn compute_streak(marks: &[Mark], now: chrono::NaiveDateTime) -> u32 {
    let read_dates: HashSet<chrono::NaiveDate> = marks.iter()
        .filter_map(|m| m.read_at.as_ref())
        .filter_map(|s| parse_datetime(s).ok())
        .map(|dt| dt.date())
        .collect();
    let mut streak = 0u32;
    let mut check = now.date();
    while read_dates.contains(&check) {
        streak += 1;
        check -= chrono::Duration::days(1);
    }
    streak
}

pub async fn stats(
    db: Arc<Database<'static>>,
    json: bool,
    since: String,
    dead_threshold: Option<f32>,
) -> Result<()> {
    let result = stats_core(db, &since, dead_threshold).await?;

    if json {
        print_json(&result);
        return Ok(());
    }

    print_activity(&result);
    print_feed_table(&result);
    print_time_chart(&result);
    print_folder_table(&result);
    print_dead_feeds(&result, dead_threshold);
    Ok(())
}

fn print_activity(r: &StatsResult) {
    println!("Reading Activity (last {} days)", r.window_days);
    println!("  Articles read:     {:>5} ({:.1}/day avg)", r.activity.articles_read, r.activity.daily_avg);
    println!("  Articles opened:   {:>5} ({:.1}% open rate)", r.activity.articles_opened, r.activity.open_rate);
    println!("  Articles starred:  {:>5} ({:.1}% star rate)", r.activity.articles_starred, r.activity.star_rate);
    println!("  Current streak:    {:>5} days", r.streak);
    println!();
}

fn print_feed_table(r: &StatsResult) {
    println!("Per-Feed Engagement");
    let rows: Vec<Vec<String>> = r.per_feed.iter().map(|f| {
        let flag = if f.is_low { " <- LOW" } else { "" };
        vec![f.feed_name.clone(), f.read.to_string(), f.opened.to_string(),
             f.skipped.to_string(), format!("{:.0}%{flag}", f.engagement_pct)]
    }).collect();
    print_table(&["FEED", "READ", "OPEN", "SKIP", "ENGAGEMENT"], &rows);
    println!();
}

fn print_time_chart(r: &StatsResult) {
    let max = r.by_hour.iter().map(|b| b.count).max().unwrap_or(1).max(1);
    println!("Time-of-Day Patterns");
    for b in &r.by_hour {
        let len = (b.count as f32 / max as f32 * 24.0) as usize;
        println!("  {:>5}  {}{}  {:>3}", b.hour_range, "#".repeat(len), ".".repeat(24 - len), b.count);
    }
    println!();
}

fn print_folder_table(r: &StatsResult) {
    if r.by_folder.is_empty() { return; }
    println!("Folder Engagement");
    let rows: Vec<Vec<String>> = r.by_folder.iter().map(|f| {
        vec![format!("{}{}", "  ".repeat(f.depth), f.folder_name), f.articles.to_string(), format!("{:.0}%", f.engagement_pct)]
    }).collect();
    print_table(&["FOLDER", "ARTICLES", "ENGAGEMENT"], &rows);
    println!();
}

fn print_dead_feeds(r: &StatsResult, threshold: Option<f32>) {
    if r.dead_feeds.is_empty() { return; }
    println!("Dead Feeds (< {:.0}% engagement)", threshold.unwrap_or(5.0));
    let rows: Vec<Vec<String>> = r.dead_feeds.iter().map(|f| {
        vec![f.feed_id.clone(), f.feed_name.clone(), format!("{:.0}%", f.engagement_pct), f.skipped.to_string()]
    }).collect();
    print_table(&["ID", "FEED", "ENGAGEMENT", "SKIPPED"], &rows);
    println!();
    println!("Consider: rssdude remove <id>");
}
