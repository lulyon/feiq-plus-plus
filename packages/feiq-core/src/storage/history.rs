//! SQLite chat history storage with pagination and search.
//! Stores all sent/received messages, grouped by contact IP.

use crate::protocol::types::{Content, Fellow, FileContent};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;

/// A stored message record
#[derive(Debug, Clone, serde::Serialize)]
pub struct MessageRecord {
    pub id: i64,
    pub contact_ip: String,
    pub contact_name: String,
    /// 0 = sent, 1 = received
    pub direction: i32,
    pub content_json: String,
    pub timestamp: i64,
}

/// Offline message pending delivery
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub id: i64,
    pub contact_ip: String,
    pub message_type: String, // "text" or "file"
    pub payload_json: String,
    pub created_at: i64,
}

/// Chat history database
pub struct HistoryDb {
    conn: Connection,
}

impl HistoryDb {
    /// Open or create the history database
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let need_init = !path.exists();
        let conn = Connection::open(path)?;

        if need_init {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contact_ip TEXT NOT NULL,
                    contact_name TEXT NOT NULL DEFAULT '',
                    direction INTEGER NOT NULL,
                    content_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_messages_contact ON messages(contact_ip, created_at);
                CREATE TABLE IF NOT EXISTS pending_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contact_ip TEXT NOT NULL,
                    message_type TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS groups_info (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    group_name TEXT NOT NULL,
                    member_ips TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )?;
        }

        Ok(Self { conn })
    }

    /// Save a message to history
    pub fn save_message(
        &self,
        contact_ip: &str,
        contact_name: &str,
        direction: i32,
        contents: &[Content],
    ) -> anyhow::Result<i64> {
        let content_json = serde_json::to_string(contents)?;
        let now = Utc::now().timestamp_millis();

        self.conn.execute(
            "INSERT INTO messages (contact_ip, contact_name, direction, content_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![contact_ip, contact_name, direction, content_json, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query messages for a contact with pagination (newest first)
    pub fn get_messages(
        &self,
        contact_ip: &str,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<MessageRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, contact_ip, contact_name, direction, content_json, created_at
             FROM messages WHERE contact_ip = ?1
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
        )?;

        let rows = stmt.query_map(params![contact_ip, limit, offset], |row| {
            Ok(MessageRecord {
                id: row.get(0)?,
                contact_ip: row.get(1)?,
                contact_name: row.get(2)?,
                direction: row.get(3)?,
                content_json: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        // Reverse to chronological order
        result.reverse();
        Ok(result)
    }

    /// Search messages across all contacts
    pub fn search_messages(&self, query: &str, limit: i64) -> anyhow::Result<Vec<MessageRecord>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, contact_ip, contact_name, direction, content_json, created_at
             FROM messages WHERE content_json LIKE ?1 OR contact_name LIKE ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(MessageRecord {
                id: row.get(0)?,
                contact_ip: row.get(1)?,
                contact_name: row.get(2)?,
                direction: row.get(3)?,
                content_json: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ─── Offline messages ─────────────────────────────────────

    /// Enqueue a pending message for offline delivery
    pub fn enqueue_pending(
        &self,
        contact_ip: &str,
        message_type: &str,
        payload_json: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO pending_messages (contact_ip, message_type, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![contact_ip, message_type, payload_json, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    /// Drain and return all pending messages for a contact (for delivery)
    pub fn drain_pending(&self, contact_ip: &str) -> anyhow::Result<Vec<PendingMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, contact_ip, message_type, payload_json, created_at
             FROM pending_messages WHERE contact_ip = ?1 ORDER BY created_at",
        )?;

        let rows = stmt.query_map(params![contact_ip], |row| {
            Ok(PendingMessage {
                id: row.get(0)?,
                contact_ip: row.get(1)?,
                message_type: row.get(2)?,
                payload_json: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }

        // Delete delivered messages
        self.conn.execute(
            "DELETE FROM pending_messages WHERE contact_ip = ?1",
            params![contact_ip],
        )?;

        Ok(result)
    }

    // ─── Groups ───────────────────────────────────────────────

    /// Save a group definition (replaces existing group with same name)
    pub fn save_group(&self, name: &str, member_ips: &[String]) -> anyhow::Result<()> {
        let members_json = serde_json::to_string(member_ips)?;
        // Delete existing group with same name to prevent duplicates
        self.conn.execute("DELETE FROM groups_info WHERE group_name = ?1", params![name])?;
        self.conn.execute(
            "INSERT INTO groups_info (group_name, member_ips, created_at)
             VALUES (?1, ?2, ?3)",
            params![name, members_json, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    /// Get all groups
    pub fn get_groups(&self) -> anyhow::Result<Vec<(String, Vec<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT group_name, member_ips FROM groups_info ORDER BY created_at",
        )?;

        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let ips_json: String = row.get(1)?;
            let ips: Vec<String> = serde_json::from_str(&ips_json).unwrap_or_default();
            Ok((name, ips))
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_crud() {
        let path = format!("/tmp/test_feix_hist_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Save a message
        let contents = vec![Content::Text {
            text: "Hello".into(),
            format: String::new(),
        }];
        let id = db
            .save_message("192.168.1.1", "Alice", 0, &contents)
            .unwrap();
        assert!(id > 0);

        // Query
        let msgs = db.get_messages("192.168.1.1", 0, 10).unwrap();
        assert!(!msgs.is_empty());
        assert_eq!(msgs[0].contact_name, "Alice");

        // Search
        let results = db.search_messages("Hello", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_pending_messages() {
        let path = format!("/tmp/test_feix_pend_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        db.enqueue_pending("10.0.0.1", "text", r#"{"text":"Hi"}"#)
            .unwrap();

        let pending = db.drain_pending("10.0.0.1").unwrap();
        assert_eq!(pending.len(), 1);

        // Should be empty now
        let empty = db.drain_pending("10.0.0.1").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_groups() {
        let path = format!("/tmp/test_feix_grp_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Clean and re-init for deterministic test
        db.save_group("Team", &["10.0.0.1".into(), "10.0.0.2".into()])
            .unwrap();
        db.save_group("Hackers", &["10.0.0.3".into()])
            .unwrap();

        let groups = db.get_groups().unwrap();
        assert!(groups.len() >= 2);
        assert_eq!(groups[0].0, "Team");

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
}
