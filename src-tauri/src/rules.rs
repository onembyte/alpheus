//! User custom scanner rules engine (~/.config/alpheus/rules.json).

use crate::scan::{du_many_kb, home, is_denied, ActionKind, Card, Tier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomRule {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tier: String, // "safe", "with-care", "manual"
    pub paths: Vec<String>,
    pub action: Option<String>, // "delete", "command", "explain"
    pub command: Option<String>,
}

fn rules_file_path() -> PathBuf {
    home().join(".config/alpheus/rules.json")
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

pub fn load_custom_rules() -> Vec<CustomRule> {
    let p = rules_file_path();
    if !p.exists() {
        return vec![];
    }
    let Ok(content) = fs::read_to_string(&p) else {
        return vec![];
    };
    serde_json::from_str::<Vec<CustomRule>>(&content).unwrap_or_default()
}

pub fn scan_custom_rules() -> Vec<Card> {
    let rules = load_custom_rules();
    let mut cards = vec![];

    for r in rules {
        let mut resolved_paths = vec![];
        for p in &r.paths {
            let expanded = expand_tilde(p);
            if !is_denied(&expanded) && expanded.exists() {
                resolved_paths.push(expanded);
            }
        }

        if resolved_paths.is_empty() && r.command.is_none() {
            continue;
        }

        let sizes = du_many_kb(&resolved_paths);
        let total_size_kb: u64 = sizes.values().sum();

        let tier = match r.tier.to_lowercase().as_str() {
            "safe" => Tier::Safe,
            "with-care" | "with_care" | "care" => Tier::WithCare,
            _ => Tier::Manual,
        };

        let action = match r.action.as_deref() {
            Some("command") => ActionKind::Command,
            Some("explain") => ActionKind::Explain,
            _ => ActionKind::Delete,
        };

        cards.push(Card {
            id: format!("custom-{}", r.id),
            title: r.title,
            description: r.description,
            tier,
            size_kb: total_size_kb,
            paths: resolved_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            proof: None,
            action,
            command_display: r.command,
        });
    }

    cards
}
