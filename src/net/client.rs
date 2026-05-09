use anyhow::{Context, Result};
use serde_json::Value;

use crate::shared::config::Config;
use crate::shared::output::*;

fn as_array(val: &Value) -> &[Value] {
    val.as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

pub struct Client {
    base_url: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Client {
    pub fn from_config(config: &Config) -> Result<Self> {
        let base_url = config
            .remote_url()
            .ok_or_else(|| anyhow::anyhow!("no server address configured"))?;
        Ok(Self {
            base_url,
            token: config.server.token.clone(),
            http: reqwest::Client::new(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.token {
            req.header("Authorization", format!("Bearer {token}"))
        } else {
            req
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let req = self.http.get(self.url(path));
        let resp = self.auth(req).send().await.context("server request failed")?;
        self.handle_response(resp).await
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let req = self.http.post(self.url(path)).json(body);
        let resp = self.auth(req).send().await.context("server request failed")?;
        self.handle_response(resp).await
    }

    async fn put(&self, path: &str, body: &Value) -> Result<Value> {
        let req = self.http.put(self.url(path)).json(body);
        let resp = self.auth(req).send().await.context("server request failed")?;
        self.handle_response(resp).await
    }

    async fn delete(&self, path: &str) -> Result<Value> {
        let req = self.http.delete(self.url(path));
        let resp = self.auth(req).send().await.context("server request failed")?;
        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let body: Value = resp.json().await.context("invalid server response")?;
        if !status.is_success() {
            let msg = body["error"].as_str().unwrap_or("unknown error");
            anyhow::bail!("server error ({status}): {msg}");
        }
        Ok(body)
    }

    // ---------------------------------------------------------------------------
    // Feed operations
    // ---------------------------------------------------------------------------

    pub async fn add_feed(
        &self,
        json: bool,
        url: String,
        tags: Vec<String>,
        folder: Option<String>,
    ) -> Result<()> {
        let resp = self
            .post(
                "/api/feeds",
                &serde_json::json!({"url": url, "tags": tags, "folder": folder}),
            )
            .await?;

        if json {
            print_json(&resp);
        } else {
            let feed = &resp["feed"];
            let title = feed["title"].as_str().unwrap_or(&url);
            let synced = resp["items_synced"].as_u64().unwrap_or(0);
            println!("Added feed: {title} ({url})");
            println!("Synced {synced} items.");
        }
        Ok(())
    }

    pub async fn list_feeds(&self, json: bool, tag: Option<String>) -> Result<()> {
        let mut path = "/api/feeds".to_string();
        if let Some(ref t) = tag {
            path = format!("{path}?tag={}", urlencoding(t));
        }
        let resp = self.get(&path).await?;

        if json {
            print_json(&resp);
        } else {
            let feeds = as_array(&resp);
            let rows: Vec<Vec<String>> = feeds
                .iter()
                .map(|f| {
                    vec![
                        f["id"].as_str().unwrap_or("").to_string(),
                        f["title"].as_str().unwrap_or("").to_string(),
                        f["url"].as_str().unwrap_or("").to_string(),
                        f["tags"]
                            .as_array()
                            .map(|t| {
                                t.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default(),
                        f["folder_name"].as_str().unwrap_or("").to_string(),
                        f["item_count"].as_u64().unwrap_or(0).to_string(),
                        match (f["error_count"].as_u64().unwrap_or(0), f["last_synced"].as_str()) {
                            (n, Some(_)) if n > 0 => format!("FAILED x{n}"),
                            (_, Some(t)) => time_ago(t),
                            (_, None) => "never".into(),
                        },
                    ]
                })
                .collect();
            print_table(
                &["ID", "TITLE", "URL", "TAGS", "FOLDER", "ITEMS", "LAST SYNCED"],
                &rows,
            );
        }
        Ok(())
    }

    pub async fn remove_feed(&self, json: bool, id: String, yes: bool) -> Result<()> {
        if !yes && !json {
            // Peek at the feed list to fetch title + item count before deleting.
            let feeds = self.get("/api/feeds").await?;
            let entry = feeds.as_array().and_then(|arr| arr.iter().find(|f| f["id"].as_str() == Some(&id)));
            let Some(entry) = entry else { anyhow::bail!("Feed not found: {id}"); };
            let title = entry["title"].as_str().unwrap_or(&id);
            let count = entry["item_count"].as_u64().unwrap_or(0);
            eprint!("Remove feed \"{title}\" and {count} items? [y/N] ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }
        let resp = self.delete(&format!("/api/feeds/{id}")).await?;
        if json {
            print_json(&resp);
        } else {
            let title = resp["title"].as_str().unwrap_or("?");
            let count = resp["items_deleted"].as_u64().unwrap_or(0);
            println!("Removed feed: {title} ({count} items deleted)");
        }
        Ok(())
    }

    pub async fn move_feed(&self, json: bool, id: String, folder_id: String) -> Result<()> {
        let resp = self
            .put(
                &format!("/api/feeds/{id}/move"),
                &serde_json::json!({"folder_id": folder_id}),
            )
            .await?;
        if json {
            print_json(&resp);
        } else {
            let ft = resp["feed_title"].as_str().unwrap_or("?");
            let fn_ = resp["folder_name"].as_str().unwrap_or("?");
            println!("Moved feed \"{ft}\" to folder \"{fn_}\"");
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Sync
    // ---------------------------------------------------------------------------

    pub async fn sync(&self, json: bool, feed: Option<String>) -> Result<()> {
        let resp = self
            .post("/api/sync", &serde_json::json!({"feed": feed}))
            .await?;
        if json {
            print_json(&resp);
        } else {
            let total = resp["new_items"].as_u64().unwrap_or(0);
            let synced = resp["feeds_synced"].as_u64().unwrap_or(0);
            if let Some(results) = resp["results"].as_array() {
                println!("Syncing {synced} feeds...");
                for r in results {
                    let title = r["title"].as_str().unwrap_or("?");
                    let new = r["new_items"].as_u64().unwrap_or(0);
                    let status = r["status"].as_str().unwrap_or("?");
                    match status {
                        "up_to_date" => println!("  {title} up to date"),
                        "error" => {
                            let err = r["error"].as_str().unwrap_or("?");
                            eprintln!("  {title}: error: {err}");
                        }
                        _ if new == 0 => println!("  {title} up to date"),
                        _ => println!("  {title}  {new} new items"),
                    }
                }
            }
            println!("Done. {total} new items.");
        }
        Ok(())
    }

    pub async fn status(&self, json: bool) -> Result<()> {
        let resp = self.get("/api/status").await?;
        if json {
            print_json(&resp);
        } else {
            let feeds = resp["feeds"].as_u64().unwrap_or(0);
            let items = resp["items"].as_u64().unwrap_or(0);
            let unread = resp["unread"].as_u64().unwrap_or(0);
            let starred = resp["starred"].as_u64().unwrap_or(0);
            let last_sync = resp["last_sync"]
                .as_str()
                .map(time_ago)
                .unwrap_or_else(|| "never".into());
            println!("Feeds: {feeds}");
            println!("Items: {items} total, {unread} unread, {starred} starred");
            println!("Last sync: {last_sync}");
            if let Some(stats) = resp["feed_stats"].as_array() {
                println!();
                let rows: Vec<Vec<String>> = stats
                    .iter()
                    .map(|s| {
                        vec![
                            s["title"].as_str().unwrap_or("?").to_string(),
                            s["last_synced"]
                                .as_str()
                                .map(time_ago)
                                .unwrap_or_else(|| "never".into()),
                            s["items"].as_u64().unwrap_or(0).to_string(),
                            s["unread"].as_u64().unwrap_or(0).to_string(),
                        ]
                    })
                    .collect();
                print_table(&["FEED", "LAST SYNCED", "ITEMS", "UNREAD"], &rows);
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Items
    // ---------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn items(
        &self,
        json: bool,
        limit: usize,
        since: Option<String>,
        tag: Option<String>,
        unread: bool,
        feed: Option<String>,
        folder: Option<String>,
    ) -> Result<()> {
        let mut params = vec![format!("limit={limit}")];
        if let Some(ref s) = since {
            params.push(format!("since={}", urlencoding(s)));
        }
        if let Some(ref t) = tag {
            params.push(format!("tag={}", urlencoding(t)));
        }
        if unread {
            params.push("unread=true".to_string());
        }
        if let Some(ref f) = feed {
            params.push(format!("feed={}", urlencoding(f)));
        }
        if let Some(ref f) = folder {
            params.push(format!("folder={}", urlencoding(f)));
        }
        let path = format!("/api/items?{}", params.join("&"));
        let resp = self.get(&path).await?;

        if json {
            print_json(&resp);
        } else {
            let items = as_array(&resp);
            print_table(&["ID", "SOURCE", "TITLE", "PUBLISHED"], &item_value_rows(items));
        }
        Ok(())
    }

    pub async fn read_item(&self, json: bool, id: String, open: bool, raw: bool) -> Result<()> {
        let resp = self.get(&format!("/api/items/{id}")).await?;
        if json {
            print_json(&resp);
        } else {
            let source = resp["source"].as_str().unwrap_or("-");
            let title = resp["title"].as_str().unwrap_or("(untitled)");
            let published = resp["published_at"].as_str().unwrap_or("-");
            let url = resp["link"].as_str().unwrap_or("-");
            let body = resp["content"]
                .as_str()
                .or(resp["summary"].as_str())
                .unwrap_or("(no content)");
            println!("Source:    {source}");
            println!("Title:     {title}");
            println!("Published: {published}");
            println!("URL:       {url}");
            println!("---");
            if raw {
                println!("{body}");
            } else {
                println!("{}", strip_html(body, 80));
            }

            if open {
                if let Some(url) = resp["link"].as_str() {
                    let _ = std::process::Command::new("open").arg(url).spawn();
                }
            }
        }
        Ok(())
    }

    pub async fn search(&self, json: bool, query: String, limit: usize) -> Result<()> {
        let path = format!(
            "/api/search?q={}&limit={limit}",
            urlencoding(&query)
        );
        let resp = self.get(&path).await?;
        if json {
            print_json(&resp);
        } else {
            let items = as_array(&resp);
            print_table(&["ID", "SOURCE", "TITLE", "PUBLISHED"], &item_value_rows(items));
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Curation
    // ---------------------------------------------------------------------------

    pub async fn mark(
        &self,
        json: bool,
        id: String,
        read: bool,
        star: bool,
        note: Option<String>,
    ) -> Result<()> {
        let resp = self
            .put(
                &format!("/api/items/{id}/mark"),
                &serde_json::json!({"read": read, "star": star, "note": note}),
            )
            .await?;
        if json {
            print_json(&resp);
        } else {
            let title = resp["item_title"].as_str().unwrap_or("(untitled)");
            let mark = &resp["mark"];
            let mut parts = Vec::new();
            if mark["read"].as_bool().unwrap_or(false) { parts.push("read"); }
            if mark["starred"].as_bool().unwrap_or(false) { parts.push("starred"); }
            if mark["note"].as_str().is_some() { parts.push("noted"); }
            let status = if parts.is_empty() { "updated".into() } else { parts.join(", ") };
            println!("Marked \"{title}\" as {status}.");
        }
        Ok(())
    }

    pub async fn starred(&self, json: bool, limit: Option<usize>) -> Result<()> {
        let mut path = "/api/starred".to_string();
        if let Some(lim) = limit {
            path = format!("{path}?limit={lim}");
        }
        let resp = self.get(&path).await?;
        if json {
            print_json(&resp);
        } else {
            let items = as_array(&resp);
            let rows: Vec<Vec<String>> = items
                .iter()
                .map(|i| {
                    vec![
                        i["id"].as_str().unwrap_or("").to_string(),
                        i["source"].as_str().unwrap_or("-").to_string(),
                        i["title"].as_str().unwrap_or("-").to_string(),
                        i["note"].as_str().unwrap_or("").to_string(),
                        i["published_at"]
                            .as_str()
                            .map(time_ago)
                            .unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();
            print_table(&["ID", "SOURCE", "TITLE", "NOTE", "PUBLISHED"], &rows);
        }
        Ok(())
    }

    pub async fn export(&self, json: bool, id: String, format: String) -> Result<()> {
        let resp = self.get(&format!("/api/export/{id}")).await?;
        if json {
            print_json(&resp);
            return Ok(());
        }
        let item: crate::shared::db::ItemJson =
            serde_json::from_value(resp).context("invalid export response")?;
        print!("{}", format_export(&item, &format)?);
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Discovery
    // ---------------------------------------------------------------------------

    pub async fn digest(&self, json: bool, since: String, tag: Option<String>) -> Result<()> {
        let mut path = format!("/api/digest?since={}", urlencoding(&since));
        if let Some(ref t) = tag {
            path.push_str(&format!("&tag={}", urlencoding(t)));
        }
        let resp = self.get(&path).await?;
        if json {
            print_json(&resp);
        } else {
            let total = resp["total"].as_u64().unwrap_or(0);
            let since_str = resp["since"].as_str().unwrap_or("?");
            println!("Digest: {total} new items since {since_str}");
            println!();
            if let Some(feeds) = resp["feeds"].as_array() {
                for f in feeds {
                    let title = f["feed_title"].as_str().unwrap_or("?");
                    let items = f["items"].as_array().map(|a| a.len()).unwrap_or(0);
                    println!("{title} ({items} new)");
                    if let Some(items) = f["items"].as_array() {
                        for item in items {
                            let t = item["title"].as_str().unwrap_or("?");
                            println!("  - {t}");
                        }
                    }
                    println!();
                }
            }
        }
        Ok(())
    }

    pub async fn trending(&self, json: bool) -> Result<()> {
        let resp = self.get("/api/trending").await?;
        if json {
            print_json(&resp);
        } else {
            let topics = as_array(&resp);
            let rows: Vec<Vec<String>> = topics
                .iter()
                .map(|t| {
                    vec![
                        t["topic"].as_str().unwrap_or("").to_string(),
                        t["mentions"].as_u64().unwrap_or(0).to_string(),
                        t["feeds"].as_u64().unwrap_or(0).to_string(),
                        t["latest"].as_str().unwrap_or("").to_string(),
                    ]
                })
                .collect();
            print_table(&["TOPIC", "MENTIONS", "FEEDS", "LATEST"], &rows);
        }
        Ok(())
    }

    pub async fn match_keywords(
        &self,
        json: bool,
        keywords: String,
        limit: usize,
        since: String,
    ) -> Result<()> {
        let path = format!(
            "/api/match?keywords={}&limit={limit}&since={}",
            urlencoding(&keywords),
            urlencoding(&since)
        );
        let resp = self.get(&path).await?;
        if json {
            print_json(&resp);
        } else {
            let items = as_array(&resp);
            let rows: Vec<Vec<String>> = items
                .iter()
                .map(|r| {
                    let item = &r["item"];
                    vec![
                        item["id"].as_str().unwrap_or("").to_string(),
                        item["source"].as_str().unwrap_or("-").to_string(),
                        item["title"].as_str().unwrap_or("-").to_string(),
                        r["matched_keywords"].as_str().unwrap_or("").to_string(),
                        item["published_at"]
                            .as_str()
                            .map(time_ago)
                            .unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();
            print_table(&["ID", "SOURCE", "TITLE", "MATCHED", "PUBLISHED"], &rows);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Stats
    // ---------------------------------------------------------------------------

    pub async fn stats(&self, json: bool, since: String, dead: Option<f32>) -> Result<()> {
        let mut path = format!("/api/stats?since={}", urlencoding(&since));
        if let Some(d) = dead {
            path.push_str(&format!("&dead={d}"));
        }
        let resp = self.get(&path).await?;
        if json {
            print_json(&resp);
        } else {
            println!("Reading Activity (last {} days)", resp["window_days"].as_u64().unwrap_or(30));
            let a = &resp["activity"];
            println!("  Articles read:     {:>5} ({:.1}/day avg)", a["articles_read"].as_u64().unwrap_or(0), a["daily_avg"].as_f64().unwrap_or(0.0));
            println!("  Articles opened:   {:>5} ({:.1}% open rate)", a["articles_opened"].as_u64().unwrap_or(0), a["open_rate"].as_f64().unwrap_or(0.0));
            println!("  Articles starred:  {:>5} ({:.1}% star rate)", a["articles_starred"].as_u64().unwrap_or(0), a["star_rate"].as_f64().unwrap_or(0.0));
            println!("  Current streak:    {:>5} days", resp["streak"].as_u64().unwrap_or(0));
            println!();
            if let Some(feeds) = resp["per_feed"].as_array() {
                let rows: Vec<Vec<String>> = feeds.iter().map(|f| {
                    let flag = if f["is_low"].as_bool().unwrap_or(false) { " <- LOW" } else { "" };
                    vec![
                        f["feed_name"].as_str().unwrap_or("?").to_string(),
                        f["read"].as_u64().unwrap_or(0).to_string(),
                        f["opened"].as_u64().unwrap_or(0).to_string(),
                        f["skipped"].as_u64().unwrap_or(0).to_string(),
                        format!("{:.0}%{flag}", f["engagement_pct"].as_f64().unwrap_or(0.0)),
                    ]
                }).collect();
                print_table(&["FEED", "READ", "OPEN", "SKIP", "ENGAGEMENT"], &rows);
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Folders
    // ---------------------------------------------------------------------------

    pub async fn folder_create(
        &self,
        json: bool,
        name: String,
        parent: Option<String>,
    ) -> Result<()> {
        let resp = self
            .post(
                "/api/folders",
                &serde_json::json!({"name": name, "parent_id": parent}),
            )
            .await?;
        if json {
            print_json(&resp);
        } else {
            let id = resp["id"].as_str().unwrap_or("?");
            let name = resp["name"].as_str().unwrap_or("?");
            println!("Created folder: {name} (id: {id})");
        }
        Ok(())
    }

    pub async fn folder_list(&self, json: bool) -> Result<()> {
        let resp = self.get("/api/folders").await?;
        if json {
            print_json(&resp);
        } else {
            crate::commands::folder::render_tree_json(&resp);
        }
        Ok(())
    }

    pub async fn folder_rename(&self, json: bool, id: String, name: String) -> Result<()> {
        let resp = self
            .put(
                &format!("/api/folders/{id}/rename"),
                &serde_json::json!({"name": name}),
            )
            .await?;
        if json {
            print_json(&resp);
        } else {
            let name = resp["name"].as_str().unwrap_or("?");
            println!("Renamed folder to: {name}");
        }
        Ok(())
    }

    pub async fn folder_move(&self, json: bool, id: String, parent: String) -> Result<()> {
        let resp = self
            .put(
                &format!("/api/folders/{id}/move"),
                &serde_json::json!({"parent_id": parent}),
            )
            .await?;
        if json {
            print_json(&resp);
        } else {
            let name = resp["name"].as_str().unwrap_or("?");
            let parent = resp["parent_id"].as_str().unwrap_or("(none)");
            println!("Moved folder: {name} -> parent {parent}");
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // OPML
    // ---------------------------------------------------------------------------

    pub async fn opml_import(&self, json: bool, file: String) -> Result<()> {
        let content = tokio::fs::read_to_string(&file)
            .await
            .with_context(|| format!("failed to read OPML file: {file}"))?;
        let resp = self.post("/api/opml", &serde_json::json!({"content": content})).await?;
        if json {
            print_json(&resp);
        } else {
            let added = resp["feeds_added"].as_u64().unwrap_or(0);
            let folders = resp["folders_created"].as_u64().unwrap_or(0);
            let skipped = resp["feeds_skipped"].as_u64().unwrap_or(0);
            println!("Imported {added} feeds, {folders} folders ({skipped} feeds skipped as duplicates).");
        }
        Ok(())
    }

    pub async fn opml_export(&self, json: bool, output: Option<String>) -> Result<()> {
        let resp = self.get("/api/opml").await?;
        if json {
            print_json(&resp);
            return Ok(());
        }
        let xml = resp["opml"].as_str().context("server returned no OPML content")?;
        match output {
            Some(path) => {
                tokio::fs::write(&path, xml)
                    .await
                    .with_context(|| format!("failed to write OPML to {path}"))?;
                eprintln!("Wrote OPML to {path}");
            }
            None => println!("{xml}"),
        }
        Ok(())
    }

    pub async fn folder_delete(&self, json: bool, id: String, recursive: bool) -> Result<()> {
        let path = if recursive {
            format!("/api/folders/{id}?recursive=true")
        } else {
            format!("/api/folders/{id}")
        };
        let resp = self.delete(&path).await?;
        if json {
            print_json(&resp);
        } else {
            let name = resp["folder"].as_str().unwrap_or("?");
            if resp["recursive"].as_bool().unwrap_or(false) {
                println!(
                    "Deleted folder: {name} ({} subfolders, {} feeds, {} items removed)",
                    resp["folders_deleted"].as_u64().unwrap_or(1) - 1,
                    resp["feeds_deleted"],
                    resp["items_deleted"]
                );
            } else {
                println!(
                    "Deleted folder: {name} ({} subfolders reparented, {} feeds reparented)",
                    resp["reparented_folders"], resp["reparented_feeds"]
                );
            }
        }
        Ok(())
    }
}

fn urlencoding(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}
