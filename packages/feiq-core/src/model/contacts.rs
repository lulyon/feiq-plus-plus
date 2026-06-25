//! Thread-safe contact book.
//! Manages the LAN user list with IP-based indexing.
//! Mirrors feiqmodel.cpp fellow management.

use crate::protocol::types::Fellow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Thread-safe contact book indexed by IP address
pub struct ContactBook {
    contacts: Vec<Fellow>,
    /// IP -> position index for fast lookup
    index: HashMap<String, usize>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(),
            index: HashMap::new(),
        }
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

    /// Find a contact by IP address
    pub fn find_by_ip(&self, ip: &str) -> Option<Fellow> {
        self.index.get(ip).map(|&idx| self.contacts[idx].clone())
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
        if let Some(idx) = self.index.get(&fellow.ip) {
            let existing = &mut self.contacts[*idx];
            existing.update(&fellow)
        } else {
            // Also check MAC match (same person, different IP)
            if !fellow.mac.is_empty() {
                if let Some(pos) = self.contacts.iter().position(|c| c.mac == fellow.mac) {
                    // Update existing with new IP
                    let old_ip = self.contacts[pos].ip.clone();
                    self.index.remove(&old_ip);
                    self.index.insert(fellow.ip.clone(), pos);
                    self.contacts[pos].ip = fellow.ip.clone();
                    return self.contacts[pos].update(&fellow);
                }
            }

            // New contact
            let idx = self.contacts.len();
            self.index.insert(fellow.ip.clone(), idx);
            self.contacts.push(fellow);
            true
        }
    }

    /// Search contacts by name, IP, host, or pinyin
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
