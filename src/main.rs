mod client;
mod commands;
mod config;
mod db;
mod feed;
mod output;
mod server;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rssdude", about = "Local RSS feed manager", version)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Add a feed
    Add {
        url: String,
        #[arg(long, action = clap::ArgAction::Append)]
        tag: Vec<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    /// List all feeds
    List {
        #[arg(long)]
        tag: Option<String>,
    },
    /// Remove a feed
    Remove {
        id: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Sync feeds
    Sync {
        #[arg(long)]
        feed: Option<String>,
    },
    /// Show sync status
    Status,
    /// List items
    Items {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        unread: bool,
        #[arg(long)]
        feed: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    /// Read an item
    Read {
        id: String,
        #[arg(long)]
        open: bool,
        /// Show raw HTML content instead of rendered text
        #[arg(long)]
        raw: bool,
    },
    /// Search items
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Mark an item
    Mark {
        id: String,
        #[arg(long)]
        read: bool,
        #[arg(long)]
        star: bool,
        #[arg(long)]
        note: Option<String>,
    },
    /// Show starred items
    Starred {
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Export an item
    Export {
        id: String,
        #[arg(long, default_value = "md")]
        format: String,
    },
    /// Daily digest
    Digest {
        #[arg(long, default_value = "24h")]
        since: String,
        #[arg(long)]
        tag: Option<String>,
    },
    /// Trending topics
    Trending,
    /// Match keywords
    Match {
        keywords: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value = "7d")]
        since: String,
    },
    /// Manage folders
    Folder {
        #[command(subcommand)]
        action: FolderAction,
    },
    /// Move a feed to a folder
    MoveFeed {
        id: String,
        #[arg(long)]
        folder: String,
    },
    /// Start server mode
    Serve {
        /// Bind address (e.g. 0.0.0.0:8484)
        #[arg(long)]
        bind: Option<String>,
    },
}

#[derive(Subcommand)]
enum FolderAction {
    /// Create a folder
    Create {
        name: String,
        #[arg(long)]
        parent: Option<String>,
    },
    /// List all folders as tree
    List,
    /// Rename a folder
    Rename { id: String, name: String },
    /// Move a folder under another
    Move {
        id: String,
        #[arg(long)]
        parent: String,
    },
    /// Delete a folder
    Delete {
        id: String,
        #[arg(long)]
        recursive: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json = cli.json;

    // Load config for server/client mode
    let config = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to load config: {e:#}");
            config::Config::default()
        }
    };

    // Handle serve command first (doesn't need client mode check)
    if let Some(Command::Serve { bind }) = &cli.command {
        let bind_addr = bind
            .clone()
            .unwrap_or_else(|| config.bind_address());
        let db = match db::open_db(&db::db_path()).await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: {e:#}");
                std::process::exit(1);
            }
        };
        if let Err(e) = server::serve(db, bind_addr, config.server.token.clone()).await {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    // If config has a remote server address, use client mode
    if config.has_remote() {
        let remote_client = match client::Client::from_config(&config) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e:#}");
                std::process::exit(1);
            }
        };
        let result = dispatch_remote(remote_client, cli.command, json).await;
        if let Err(e) = result {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    // Standalone mode — open local DB
    let db = match db::open_db(&db::db_path()).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };

    let result: Result<()> = match cli.command {
        None => {
            if json {
                Err(anyhow::anyhow!("--json requires a subcommand"))
            } else {
                tui::run(db).await
            }
        }
        Some(Command::Serve { .. }) => unreachable!(),
        Some(Command::MoveFeed { id, folder }) => {
            commands::feed_mgmt::move_to_folder(db, json, id, folder).await
        }
        Some(Command::Add { url, tag, folder }) => {
            commands::feed_mgmt::add(db, json, url, tag, folder).await
        }
        Some(Command::List { tag }) => commands::feed_mgmt::list(db, json, tag).await,
        Some(Command::Remove { id, yes }) => commands::feed_mgmt::remove(db, json, id, yes).await,
        Some(Command::Sync { feed }) => commands::sync::sync(db, json, feed).await,
        Some(Command::Status) => commands::sync::status(db, json).await,
        Some(Command::Items {
            limit,
            since,
            tag,
            unread,
            feed,
            folder,
        }) => {
            commands::read::items(
                db,
                json,
                commands::read::ItemsQuery {
                    limit,
                    since,
                    tag,
                    unread,
                    feed_id: feed,
                    folder_id: folder,
                },
            )
            .await
        }
        Some(Command::Read { id, open, raw }) => {
            commands::read::read_item(db, json, id, open, raw).await
        }
        Some(Command::Search { query, limit }) => {
            commands::read::search(db, json, query, limit).await
        }
        Some(Command::Mark {
            id,
            read,
            star,
            note,
        }) => commands::curate::mark(db, json, id, read, star, note).await,
        Some(Command::Starred { limit }) => commands::curate::starred(db, json, limit).await,
        Some(Command::Export { id, format }) => {
            commands::curate::export(db, json, id, format).await
        }
        Some(Command::Digest { since, tag }) => {
            commands::discover::digest(db, json, since, tag).await
        }
        Some(Command::Trending) => commands::discover::trending(db, json).await,
        Some(Command::Match {
            keywords,
            limit,
            since,
        }) => commands::discover::match_keywords(db, json, keywords, limit, since).await,
        Some(Command::Folder { action }) => match action {
            FolderAction::Create { name, parent } => {
                commands::folder::create(db, json, name, parent).await
            }
            FolderAction::List => commands::folder::list(db, json).await,
            FolderAction::Rename { id, name } => commands::folder::rename(db, json, id, name).await,
            FolderAction::Move { id, parent } => {
                commands::folder::move_folder(db, json, id, parent).await
            }
            FolderAction::Delete { id, recursive } => {
                commands::folder::delete(db, json, id, recursive).await
            }
        },
    };

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Dispatch commands to the remote server via HTTP client.
async fn dispatch_remote(c: client::Client, command: Option<Command>, json: bool) -> Result<()> {
    match command {
        None => {
            // TUI in client mode not yet supported
            anyhow::bail!("TUI mode is not supported in client mode. Use CLI commands or run standalone.")
        }
        Some(Command::Serve { .. }) => unreachable!(),
        Some(Command::Add { url, tag, folder }) => c.add_feed(json, url, tag, folder).await,
        Some(Command::List { tag }) => c.list_feeds(json, tag).await,
        Some(Command::Remove { id, yes: _ }) => c.remove_feed(json, id).await,
        Some(Command::MoveFeed { id, folder }) => c.move_feed(json, id, folder).await,
        Some(Command::Sync { feed }) => c.sync(json, feed).await,
        Some(Command::Status) => c.status(json).await,
        Some(Command::Items {
            limit,
            since,
            tag,
            unread,
            feed,
            folder,
        }) => c.items(json, limit, since, tag, unread, feed, folder).await,
        Some(Command::Read { id, open, raw: _ }) => c.read_item(json, id, open).await,
        Some(Command::Search { query, limit }) => c.search(json, query, limit).await,
        Some(Command::Mark {
            id,
            read,
            star,
            note,
        }) => c.mark(json, id, read, star, note).await,
        Some(Command::Starred { limit }) => c.starred(json, limit).await,
        Some(Command::Export { id, format }) => c.export(json, id, format).await,
        Some(Command::Digest { since, tag: _ }) => c.digest(json, since).await,
        Some(Command::Trending) => c.trending(json).await,
        Some(Command::Match {
            keywords,
            limit,
            since,
        }) => c.match_keywords(json, keywords, limit, since).await,
        Some(Command::Folder { action }) => match action {
            FolderAction::Create { name, parent } => c.folder_create(json, name, parent).await,
            FolderAction::List => c.folder_list(json).await,
            FolderAction::Rename { id, name } => c.folder_rename(json, id, name).await,
            FolderAction::Move { id, parent } => c.folder_move(json, id, parent).await,
            FolderAction::Delete { id, recursive } => c.folder_delete(json, id, recursive).await,
        },
    }
}
