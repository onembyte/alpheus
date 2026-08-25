use std::process::Command;

#[derive(serde::Serialize, Clone, Copy)]
pub struct DiskUsage {
    pub total_kb: u64,
    pub free_kb: u64,
}

/// Root/Home-volume usage via `df -k` — zero extra dependencies.
pub fn usage() -> DiskUsage {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let try_df = |path: &str| -> Option<DiskUsage> {
        let out = Command::new("df")
            .args(["-k", path])
            .output()
            .or_else(|_| Command::new("/bin/df").args(["-k", path]).output())
            .or_else(|_| Command::new("/usr/bin/df").args(["-k", path]).output())
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().nth(1)?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 {
            Some(DiskUsage {
                total_kb: fields[1].parse().unwrap_or(0),
                free_kb: fields[3].parse().unwrap_or(0),
            })
        } else {
            None
        }
    };

    try_df(&home)
        .or_else(|| try_df("/"))
        .unwrap_or(DiskUsage {
            total_kb: 0,
            free_kb: 0,
        })
}
