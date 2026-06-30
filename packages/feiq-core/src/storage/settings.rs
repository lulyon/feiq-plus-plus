//! Application configuration (~/.feiq_setting.ini equivalent).
//! Supports the same INI format as original feiq for migration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Connection mode for the engine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionMode {
    /// Pure LAN UDP broadcast (default)
    #[serde(rename = "lan")]
    LanOnly,
    /// Relay server only
    #[serde(rename = "relay")]
    RelayOnly,
    /// LAN + Relay simultaneously, LAN preferred
    #[serde(rename = "hybrid")]
    Hybrid,
}

impl Default for ConnectionMode {
    fn default() -> Self {
        Self::LanOnly
    }
}

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
    /// Connection mode: LAN, Relay, or Hybrid
    #[serde(default)]
    pub mode: ConnectionMode,
    /// Relay server WebSocket URL (e.g. ws://server:2426)
    #[serde(default)]
    pub relay_server_url: String,
    /// Relay room name
    #[serde(default = "default_relay_room")]
    pub relay_room: String,
    /// Theme: "auto", "light", or "dark"
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Shared directory path for file sharing (Phase 5.3)
    #[serde(default)]
    pub shared_dir: String,
    /// Optional password for accessing the shared directory
    #[serde(default)]
    pub shared_dir_password: String,
}

fn default_theme() -> String { "auto".to_string() }

fn default_port() -> u16 { 2425 }
fn default_name() -> String { "feiq_user".to_string() }
fn default_host() -> String { "feiq++".to_string() }
fn default_title() -> String { "feiq++".to_string() }
fn default_true() -> bool { true }
fn default_relay_room() -> String { "default".to_string() }

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
            mode: ConnectionMode::default(),
            relay_server_url: String::new(),
            relay_room: default_relay_room(),
            theme: default_theme(),
            shared_dir: String::new(),
            shared_dir_password: String::new(),
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
                    "network/mode" | "mode" => {
                        config.mode = match value {
                            "relay" => ConnectionMode::RelayOnly,
                            "hybrid" => ConnectionMode::Hybrid,
                            _ => ConnectionMode::LanOnly,
                        };
                    }
                    "network/relay_server_url" | "relay_server_url" => {
                        config.relay_server_url = value.to_string();
                    }
                    "network/relay_room" | "relay_room" => {
                        config.relay_room = value.to_string();
                    }
                    "app/theme" | "theme" => {
                        config.theme = value.to_string();
                    }
                    "share/shared_dir" | "shared_dir" => {
                        config.shared_dir = value.to_string();
                    }
                    "share/shared_dir_password" | "shared_dir_password" => {
                        config.shared_dir_password = value.to_string();
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
        let mode_str = match self.mode {
            ConnectionMode::LanOnly => "lan",
            ConnectionMode::RelayOnly => "relay",
            ConnectionMode::Hybrid => "hybrid",
        };
        content.push_str(&format!("mode = {mode_str}\n"));
        if !self.relay_server_url.is_empty() {
            content.push_str(&format!("relay_server_url = {}\n", self.relay_server_url));
        }
        content.push_str(&format!("relay_room = {}\n", self.relay_room));
        content.push_str(&format!("theme = {}\n", self.theme));
        content.push_str("\n[share]\n");
        content.push_str(&format!("shared_dir = {}\n", self.shared_dir));
        if !self.shared_dir_password.is_empty() {
            content.push_str(&format!("shared_dir_password = {}\n", self.shared_dir_password));
        }
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
