//! Thread-safe contact book.
//! Manages the LAN user list with IP-based indexing.
//! Mirrors feiqmodel.cpp fellow management.

use crate::protocol::types::Fellow;
use pinyin::ToPinyin;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Check if a query matches the pinyin representation of a name.
/// Supports both full pinyin (e.g., "zhangsan") and first-letter (e.g., "zs") matching.
fn pinyin_matches(name: &str, lower_query: &str) -> bool {
    let mut first_letters = String::new();
    let mut full_pinyin = String::new();

    for (i, pinyin) in name.to_pinyin().enumerate() {
        match pinyin {
            Some(p) => {
                first_letters.push_str(p.first_letter());
                full_pinyin.push_str(p.plain());
            }
            None => {
                // Non-CJK character: include as-is for mixed searches
                let ch = name.chars().nth(i).unwrap();
                first_letters.push(ch.to_ascii_lowercase());
                full_pinyin.push(ch.to_ascii_lowercase());
            }
        }
    }

    first_letters.contains(lower_query) || full_pinyin.contains(lower_query)
}

/// Thread-safe contact book indexed by ip:port
pub struct ContactBook {
    contacts: Vec<Fellow>,
    /// ip:port -> position index for fast lookup
    index: HashMap<String, usize>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Build index key from ip and port
    fn key(ip: &str, port: u16) -> String {
        format!("{}:{}", ip, port)
    }

    /// Clone as Arc<Mutex<Self>> for sharing across tasks
    pub fn clone_arc(&self) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            contacts: self.contacts.clone(),
            index: self.index.clone(),
        }))
    }

    /// Get all contacts sorted by display name
    pub fn all(&self) -> Vec<Fellow> {
        let mut contacts = self.contacts.clone();
        contacts.sort_by(|a, b| a.display_name().cmp(b.display_name()));
        contacts
    }

    /// Find a contact by ip:port (O(1) via index)
    pub fn find(&self, ip: &str, port: u16) -> Option<Fellow> {
        self.index
            .get(&Self::key(ip, port))
            .map(|&idx| self.contacts[idx].clone())
    }

    /// Find a contact by IP address (linear scan fallback for DHCP changes / legacy callers)
    pub fn find_by_ip(&self, ip: &str) -> Option<Fellow> {
        self.contacts.iter().find(|c| c.ip == ip).cloned()
    }

    /// Find a contact by IP or MAC (same identity check)
    pub fn find_same(&self, fellow: &Fellow) -> Option<Fellow> {
        self.find_by_ip(&fellow.ip).or_else(|| {
            if !fellow.mac.is_empty() {
                self.contacts.iter().find(|c| c.mac == fellow.mac).cloned()
            } else {
                None
            }
        })
    }

    /// Insert or update a contact. Returns true if changed/new.
    pub fn upsert(&mut self, fellow: Fellow) -> bool {
        let k = Self::key(&fellow.ip, fellow.port);
        if let Some(idx) = self.index.get(&k) {
            let existing = &mut self.contacts[*idx];
            existing.update(&fellow)
        } else {
            // Also check MAC match (same person, different ip:port)
            if !fellow.mac.is_empty() {
                if let Some(pos) = self.contacts.iter().position(|c| c.mac == fellow.mac) {
                    // Update index with new ip:port key
                    let old_key = Self::key(&self.contacts[pos].ip, self.contacts[pos].port);
                    self.index.remove(&old_key);
                    self.index.insert(k, pos);
                    self.contacts[pos].ip = fellow.ip.clone();
                    self.contacts[pos].port = fellow.port;
                    return self.contacts[pos].update(&fellow);
                }
            }

            // New contact
            let idx = self.contacts.len();
            self.index.insert(k, idx);
            self.contacts.push(fellow);
            true
        }
    }

    /// Search contacts by name, IP, host, pc_name, or pinyin
    pub fn search(&self, query: &str) -> Vec<Fellow> {
        if query.is_empty() {
            return self.all();
        }
        let lower = query.to_lowercase();
        self.contacts
            .iter()
            .filter(|c| {
                c.display_name().to_lowercase().contains(&lower)
                    || c.ip.contains(&lower)
                    || c.host.to_lowercase().contains(&lower)
                    || c.pc_name.to_lowercase().contains(&lower)
                    || pinyin_matches(c.display_name(), &lower)
            })
            .cloned()
            .collect()
    }

    /// Count contacts
    pub fn count(&self) -> usize {
        self.contacts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_new() {
        let mut book = ContactBook::new();
        let fellow = Fellow::new("192.168.1.1");
        assert!(book.upsert(fellow));
        assert_eq!(book.count(), 1);
    }

    #[test]
    fn test_upsert_update() {
        let mut book = ContactBook::new();
        let mut fellow = Fellow::new("192.168.1.1");
        fellow.name = "Alice".to_string();
        book.upsert(fellow.clone());

        let mut updated = Fellow::new("192.168.1.1");
        updated.name = "Alice2".to_string();
        assert!(book.upsert(updated));
        assert_eq!(book.count(), 1);
        assert_eq!(book.find_by_ip("192.168.1.1").unwrap().name, "Alice2");
    }

    #[test]
    fn test_mac_match_different_ip() {
        let mut book = ContactBook::new();
        let mut f1 = Fellow::new("10.0.0.1");
        f1.mac = "AABBCCDDEEFF".to_string();
        f1.name = "Bob".to_string();
        book.upsert(f1);

        // Same MAC, different IP -> should update
        let mut f2 = Fellow::new("10.0.0.2");
        f2.mac = "AABBCCDDEEFF".to_string();
        f2.name = "Bob2".to_string();
        book.upsert(f2);

        assert_eq!(book.count(), 1);
    }
}
