//! Ollama models: one review card per model, sized from the layers only it uses.
//!
//! A model is a manifest (`manifests/<host>/<namespace>/<model>/<tag>`) naming
//! content-addressed blobs (`blobs/sha256-<hex>`). Models share blobs, a
//! fine-tune and its base above all, so a card offers a model's manifest plus
//! only the blobs no other manifest names. If any manifest can't be read, its
//! layers are unknown and nothing is offered. Ollama prunes blobs no manifest
//! names when its server starts, so a shared layer goes once its last model does.

use crate::exec::fmt;
use crate::scan::{du_many_kb, home, is_denied, modified_day, ActionKind, Card, Tier};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Model {
    /// As `ollama list` shows it: `gpt-oss:latest`, `hf.co/user/repo:q4`.
    pub name: String,
    pub manifest: PathBuf,
    /// The manifest plus every blob on disk that only this model uses.
    pub paths: Vec<PathBuf>,
    /// Layers left in place because another model uses them too.
    pub shared: usize,
}

#[derive(Deserialize)]
struct Layer {
    digest: String,
}

#[derive(Deserialize)]
struct Manifest {
    config: Option<Layer>,
    #[serde(default)]
    layers: Vec<Layer>,
}

/// `$OLLAMA_MODELS` when set, else Ollama's default under the home folder.
pub fn models_dir(env: Option<String>, home: &Path) -> PathBuf {
    match env {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => home.join(".ollama/models"),
    }
}

/// The blob file for a digest. Only `sha256:` plus 64 hex characters is
/// accepted: the digest becomes a file name, so nothing else may reach a path.
pub fn blob_for(blobs: &Path, digest: &str) -> Option<PathBuf> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| blobs.join(format!("sha256-{hex}")))
}

/// Every model under `dir`, or none at all if any manifest can't be read.
pub fn models(dir: &Path) -> Vec<Model> {
    let root = dir.join("manifests");
    let blobs = dir.join("blobs");
    let mut files = vec![];
    walk(&root, &mut files);

    let mut parsed: Vec<(String, PathBuf, Vec<PathBuf>)> = vec![];
    for f in files {
        let name = model_name(&root, &f);
        let layers = fs::read_to_string(&f)
            .ok()
            .and_then(|json| layers(&json, &blobs));
        match (name, layers) {
            (Some(name), Some(layers)) => parsed.push((name, f, layers)),
            _ => return vec![],
        }
    }

    let mut uses: HashMap<PathBuf, usize> = HashMap::new();
    for (_, _, layers) in &parsed {
        for l in layers {
            *uses.entry(l.clone()).or_default() += 1;
        }
    }

    parsed
        .into_iter()
        .map(|(name, manifest, layers)| {
            let mut paths = vec![manifest.clone()];
            let mut shared = 0;
            for l in layers {
                if uses[&l] > 1 {
                    shared += 1;
                } else if l.exists() {
                    paths.push(l);
                }
            }
            Model {
                name,
                manifest,
                paths,
                shared,
            }
        })
        .collect()
}

/// The blobs a manifest names, deduplicated, or `None` if any digest is bad.
fn layers(json: &str, blobs: &Path) -> Option<Vec<PathBuf>> {
    let m: Manifest = serde_json::from_str(json).ok()?;
    let mut out = m
        .config
        .iter()
        .chain(m.layers.iter())
        .map(|l| blob_for(blobs, &l.digest))
        .collect::<Option<Vec<_>>>()?;
    out.sort();
    out.dedup();
    Some(out)
}

/// `host/namespace/model/tag` → the name `ollama list` prints.
fn model_name(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let [host, namespace, model, tag] = parts.as_slice() else {
        return None;
    };
    Some(match (host.as_str(), namespace.as_str()) {
        ("registry.ollama.ai", "library") => format!("{model}:{tag}"),
        ("registry.ollama.ai", _) => format!("{namespace}/{model}:{tag}"),
        _ => format!("{host}/{namespace}/{model}:{tag}"),
    })
}

/// Every manifest file under `dir`. Dotfiles (Finder's `.DS_Store`) are not
/// manifests and would otherwise read as broken ones.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&e.path(), out);
        } else {
            out.push(e.path());
        }
    }
}

pub fn cards(dir: &Path) -> Vec<Card> {
    let models = models(dir);
    let all: Vec<PathBuf> = models.iter().flat_map(|m| m.paths.clone()).collect();
    let sizes = du_many_kb(&all);

    models
        .iter()
        // Half a model is a broken model: offer all of its paths or none.
        .filter(|m| !m.paths.iter().any(|p| is_denied(p)))
        .filter_map(|m| {
            let kb: u64 = m.paths.iter().filter_map(|p| sizes.get(p)).sum();
            if kb < 1024 {
                return None;
            }
            let mut proof = vec![format!(
                "{}  ·  {}  ·  updated {}",
                fmt(kb),
                m.name,
                modified_day(&m.manifest)
            )];
            if m.shared > 0 {
                proof.push(format!(
                    "kept  ·  {} layer(s) another model also uses",
                    m.shared
                ));
            }
            let slug: String = m
                .name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' })
                .collect();
            Some(Card {
                id: format!("ollama-model-{slug}"),
                title: format!("Ollama model {}", m.name),
                description: "Weights for a local model. A library model comes back with `ollama pull <name>`; one you built with `ollama create` can't be pulled again, so keep a copy first. Layers another model also uses are kept.".into(),
                tier: Tier::WithCare,
                size_kb: kb,
                paths: m.paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                proof: Some(proof.join("\n")),
                action: ActionKind::Delete,
                command_display: None,
            })
        })
        .collect()
}

pub fn scan() -> Vec<Card> {
    cards(&models_dir(std::env::var("OLLAMA_MODELS").ok(), &home()))
}
