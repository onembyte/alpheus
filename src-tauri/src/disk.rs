use std::process::Command;

#[derive(serde::Serialize, Clone, Copy)]
pub struct DiskUsage {
    pub total_kb: u64,
    pub free_kb: u64,
}

/// Root-volume usage via `df -k /` — zero extra dependencies.
pub fn usage() -> DiskUsage {
    if let Ok(out) = Command::new("/bin/df").args(["-k", "/"]).output() {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = text.lines().nth(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 4 {
                return DiskUsage {
                    total_kb: fields[1].parse().unwrap_or(0),
                    free_kb: fields[3].parse().unwrap_or(0),
                };
            }
        }
    }
    DiskUsage {
        total_kb: 0,
        free_kb: 0,
    }
}
