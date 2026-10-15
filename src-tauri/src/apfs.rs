//! APFS volumes of the startup disk: the instant top level of the storage
//! detail view.
//!
//! APFS already knows how much each volume holds (macOS itself, Preboot,
//! Recovery, the VM swap volume, the Data volume), so this level costs two
//! `diskutil` calls instead of a walk. `diskutil` speaks XML plists; `plutil`
//! turns them into JSON, so no plist parser is needed.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Volume {
    pub id: String,
    pub name: String,
    /// APFS roles: System, Data, Preboot, Recovery, Update, VM. Empty for a
    /// volume the user created.
    pub roles: Vec<String>,
    pub used_kb: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Container {
    pub reference: String,
    pub capacity_kb: u64,
    pub free_kb: u64,
    pub volumes: Vec<Volume>,
}

impl Container {
    pub fn role(&self, role: &str) -> Option<&Volume> {
        self.volumes
            .iter()
            .find(|v| v.roles.iter().any(|r| r == role))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawList {
    containers: Vec<RawContainer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawContainer {
    container_reference: String,
    capacity_ceiling: u64,
    capacity_free: u64,
    volumes: Vec<RawVolume>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawVolume {
    device_identifier: String,
    name: String,
    #[serde(default)]
    roles: Vec<String>,
    capacity_in_use: u64,
}

/// The first container in `diskutil apfs list -plist` output, converted to JSON.
pub fn parse_container(json: &str) -> Option<Container> {
    let list: RawList = serde_json::from_str(json).ok()?;
    let c = list.containers.into_iter().next()?;
    Some(Container {
        reference: c.container_reference,
        capacity_kb: c.capacity_ceiling / 1024,
        free_kb: c.capacity_free / 1024,
        volumes: c
            .volumes
            .into_iter()
            .map(|v| Volume {
                id: v.device_identifier,
                name: v.name,
                roles: v.roles,
                used_kb: v.capacity_in_use / 1024,
            })
            .collect(),
    })
}

/// The container holding `/`, or `None` off macOS or when `/` isn't APFS.
pub fn read_root() -> Option<Container> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct RootInfo {
        #[serde(rename = "APFSContainerReference")]
        container: String,
    }
    let info: RootInfo =
        serde_json::from_str(&plist_json(&["/usr/sbin/diskutil", "info", "-plist", "/"])?).ok()?;
    if !info.container.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    parse_container(&plist_json(&[
        "/usr/sbin/diskutil",
        "apfs",
        "list",
        "-plist",
        &info.container,
    ])?)
}

/// Runs `argv`, whose stdout is an XML plist, and returns it as JSON.
fn plist_json(argv: &[&str]) -> Option<String> {
    let out = Command::new(argv[0]).args(&argv[1..]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let mut plutil = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    plutil.stdin.take()?.write_all(&out.stdout).ok()?;
    let json = plutil.wait_with_output().ok()?;
    json.status
        .success()
        .then(|| String::from_utf8_lossy(&json.stdout).to_string())
}
