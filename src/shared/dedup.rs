use crate::shared::db::ItemJson;
use strsim::sorensen_dice;

/// Normalize a title for comparison: lowercase, strip punctuation, collapse whitespace.
fn normalize_title(s: &str) -> String {
    s.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Deduplicate items by title similarity (Sorensen-Dice > 0.8).
/// Returns items with duplicate count. First-seen item is kept.
pub fn dedup_items(items: Vec<ItemJson>) -> Vec<(ItemJson, usize)> {
    let mut result: Vec<(ItemJson, String, usize)> = Vec::new(); // (item, normalized_title, dup_count)

    for item in items {
        let title_norm = normalize_title(item.title.as_deref().unwrap_or(""));
        if title_norm.is_empty() {
            result.push((item, String::new(), 0));
            continue;
        }
        let mut is_dup = false;
        for (_, kept_norm, dup_count) in &mut result {
            if !kept_norm.is_empty() && sorensen_dice(&title_norm, kept_norm) > 0.8 {
                *dup_count += 1;
                is_dup = true;
                break;
            }
        }
        if !is_dup {
            result.push((item, title_norm, 0));
        }
    }

    result.into_iter().map(|(item, _, count)| (item, count)).collect()
}
