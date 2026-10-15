//! Ollama models: one review card per model, sized from the layers only that
//! model uses. Models share layers (a fine-tune and its base, a common
//! license blob), so deleting every blob a manifest names would break the
//! models that remain.

use alpheus_lib::exec;
use alpheus_lib::ollama;
use alpheus_lib::scan::{self, ActionKind, Tier};
use std::fs;
use std::path::{Path, PathBuf};

fn hex(c: char) -> String {
    c.to_string().repeat(64)
}

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_ollama_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("blobs")).unwrap();
    root
}

fn blob(models: &Path, c: char, kb: usize) {
    fs::write(
        models.join("blobs").join(format!("sha256-{}", hex(c))),
        vec![0u8; kb * 1024],
    )
    .unwrap();
}

fn manifest(models: &Path, rel: &str, config: char, layers: &[char]) {
    let layer = |c: &char| format!(r#"{{"digest":"sha256:{}","size":1}}"#, hex(*c));
    let json = format!(
        r#"{{"schemaVersion":2,"config":{},"layers":[{}]}}"#,
        layer(&config),
        layers.iter().map(layer).collect::<Vec<_>>().join(",")
    );
    let p = models.join("manifests").join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, json).unwrap();
}

/// Two library models sharing layer `e`, plus a Hugging Face model.
fn three_models(name: &str) -> PathBuf {
    let m = fixture(name);
    for (c, kb) in [
        ('a', 3072),
        ('b', 2048),
        ('e', 1024),
        ('c', 2048),
        ('1', 1),
        ('2', 1),
        ('3', 1),
    ] {
        blob(&m, c, kb);
    }
    manifest(
        &m,
        "registry.ollama.ai/library/gpt-oss/latest",
        '1',
        &['a', 'e'],
    );
    manifest(
        &m,
        "registry.ollama.ai/library/planner/v16",
        '2',
        &['b', 'e'],
    );
    manifest(&m, "hf.co/someone/tiny/q4", '3', &['c']);
    m
}

fn blob_path(m: &Path, c: char) -> PathBuf {
    m.join("blobs").join(format!("sha256-{}", hex(c)))
}

#[test]
fn test_models_are_named_like_ollama_list() {
    let m = three_models("names");
    let mut names: Vec<String> = ollama::models(&m).iter().map(|x| x.name.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        ["gpt-oss:latest", "hf.co/someone/tiny:q4", "planner:v16"]
    );
    let _ = fs::remove_dir_all(m);
}

/// A layer two models use belongs to neither card: removing one model must
/// leave the other runnable.
#[test]
fn test_shared_layers_are_never_offered() {
    let m = three_models("shared");
    let models = ollama::models(&m);
    let gpt = models.iter().find(|x| x.name == "gpt-oss:latest").unwrap();

    assert!(gpt.paths.contains(&blob_path(&m, 'a')));
    assert!(
        gpt.paths.contains(&blob_path(&m, '1')),
        "its config blob goes too"
    );
    assert!(
        gpt.paths
            .contains(&m.join("manifests/registry.ollama.ai/library/gpt-oss/latest")),
        "the manifest goes too, or `ollama list` shows a broken model"
    );
    for x in &models {
        assert!(
            !x.paths.contains(&blob_path(&m, 'e')),
            "{} offered a shared layer",
            x.name
        );
    }
    let _ = fs::remove_dir_all(m);
}

/// One unreadable manifest means its layers are unknown, and any of them could
/// be a layer another card thinks it owns alone. Nothing may be offered.
#[test]
fn test_unreadable_manifest_offers_nothing() {
    let m = three_models("broken");
    let p = m.join("manifests/registry.ollama.ai/library/broken/latest");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, "{ not json").unwrap();
    assert!(ollama::models(&m).is_empty());
    let _ = fs::remove_dir_all(m);
}

/// A digest is turned into a file name, so anything but sha256 + 64 hex
/// characters must be refused rather than joined onto the blobs path.
#[test]
fn test_digest_must_be_plain_sha256() {
    let blobs = Path::new("/m/blobs");
    assert_eq!(
        ollama::blob_for(blobs, &format!("sha256:{}", hex('a'))),
        Some(blobs.join(format!("sha256-{}", hex('a'))))
    );
    assert_eq!(ollama::blob_for(blobs, "sha256:../../../.ssh/id_rsa"), None);
    assert_eq!(
        ollama::blob_for(blobs, &format!("sha256:{}", hex('g'))),
        None
    );
    assert_eq!(ollama::blob_for(blobs, &format!("md5:{}", hex('a'))), None);
    assert_eq!(ollama::blob_for(blobs, "sha256:abc"), None);
}

#[test]
fn test_models_dir_honours_ollama_models() {
    let h = Path::new("/Users/me");
    assert_eq!(
        ollama::models_dir(None, h),
        PathBuf::from("/Users/me/.ollama/models")
    );
    assert_eq!(
        ollama::models_dir(Some("/Volumes/fast/models".into()), h),
        PathBuf::from("/Volumes/fast/models")
    );
    assert_eq!(
        ollama::models_dir(Some(String::new()), h),
        PathBuf::from("/Users/me/.ollama/models")
    );
}

#[test]
fn test_cards_need_review_and_go_to_trash() {
    let m = three_models("cards");
    let cards = ollama::cards(&m);
    assert_eq!(cards.len(), 3);
    for card in &cards {
        assert!(card.id.starts_with("ollama-model-"), "{}", card.id);
        assert!(card.tier == Tier::WithCare, "{} must need review", card.id);
        assert!(card.action == ActionKind::Delete);
        assert!(!exec::RM_ALLOWED.contains(&card.id.as_str()));
        assert!(card.proof.is_some());
        assert!(card.size_kb > 0);
    }
    let gpt = cards
        .iter()
        .find(|c| c.title.contains("gpt-oss:latest"))
        .unwrap();
    assert!(
        gpt.size_kb >= 3072 && gpt.size_kb < 3072 + 1024,
        "sized from its own layers only, got {} KB",
        gpt.size_kb
    );
    let _ = fs::remove_dir_all(m);
}
