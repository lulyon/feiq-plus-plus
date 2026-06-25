//! Application configuration (~/.feiq_setting.ini equivalent).
//! Supports the same INI format as original feiq for migration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration, serializable for INI storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// User display name (required)
    #[serde(default = "default_name")]
    pub name: String,
    /// Host / machine name
    #[serde(default = "default_host")]
    pub host: String,
    /// Window title
    #[serde(default = "default_title")]
    pub title: String,
    /// true = Enter sends, Ctrl+Enter newlines; false = reversed
    #[serde(default = "default_true")]
    pub send_by_enter: bool,
    /// Custom broadcast IP prefixes (for cross-subnet discovery)
    #[serde(default)]
    pub custom_group: String,
    /// Resolved list of IPs from custom_group
    #[serde(default)]
    pub custom_ips: Vec<String>,
    /// Enable contact ranking by communication frequency
    #[serde(default = "default_true")]
    pub rank_user_enable: bool,
    /// UDP/TCP port (default 2425). Change for multi-instance testing.
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 { 2425 }
fn default_name() -> String { "feiq_user".to_string() }
fn default_host() -> String { "feiq++".to_string() }
fn default_title() -> String { "feiq++".to_string() }
fn default_true() -> bool { true }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            host: default_host(),
            title: default_title(),
            send_by_enter: true,
            custom_group: String::new(),
            custom_ips: Vec::new(),
            rank_user_enable: true,
            port: 2425,
        }
    }
}

impl AppConfig {
    /// Load from a standard INI file (like ~/.feiq_setting.ini)
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(path)?;
        let mut config = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();
                match key {
                    "user/name" | "name" => config.name = value.to_string(),
                    "user/host" | "host" => config.host = value.to_string(),
                    "app/title" | "title" => config.title = value.to_string(),
                    "app/send_by_enter" | "send_by_enter" => {
                        config.send_by_enter = value != "0";
                    }
                    "network/custom_group" | "custom_group" => {
                        config.custom_group = value.to_string();
                        // Resolve IP ranges: "192.168.74.|192.168.82." -> individual IPs
                        config.custom_ips = resolve_ip_ranges(value);
                    }
                    "rank_user/enable" | "rank_user_enable" => {
                        config.rank_user_enable = value != "0";
                    }
                    _ => {}
                }
            }
        }

        Ok(config)
    }

    /// Save to INI file
    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        let mut content = String::new();
        content.push_str("[user]\n");
        content.push_str(&format!("name = {}\n", self.name));
        content.push_str(&format!("host = {}\n", self.host));
        content.push_str("\n[app]\n");
        content.push_str(&format!("title = {}\n", self.title));
        content.push_str(&format!("send_by_enter = {}\n", if self.send_by_enter { "1" } else { "0" }));
        content.push_str("\n[network]\n");
        content.push_str(&format!("custom_group = {}\n", self.custom_group));
        content.push_str("\n[rank_user]\n");
        content.push_str(&format!("enable = {}\n", if self.rank_user_enable { "1" } else { "0" }));
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Resolve IP range pattern like "192.168.74.|192.168.82." to individual IPs
fn resolve_ip_ranges(spec: &str) -> Vec<String> {
    let mut ips = Vec::new();
    for segment in spec.split('|') {
        let segment = segment.trim();
        if segment.ends_with('.') {
            let prefix = segment;
            for i in 2..254 {
                ips.push(format!("{prefix}{i}"));
            }
        } else if !segment.is_empty() {
            ips.push(segment.to_string());
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ip_ranges() {
        let spec = "192.168.74.|192.168.82.";
        let ips = resolve_ip_ranges(spec);
        assert_eq!(ips.len(), 252 * 2);
        assert_eq!(ips[0], "192.168.74.2");
        assert_eq!(ips[252], "192.168.82.2");
    }
}
