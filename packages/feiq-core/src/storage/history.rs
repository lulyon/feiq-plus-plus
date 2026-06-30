//! SQLite chat history storage with pagination and search.
//! Stores all sent/received messages, grouped by contact IP.

use crate::protocol::types::Content;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashMap;
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

        // Always ensure contact_meta table exists (for existing DBs without it)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS contact_meta (
                ip TEXT PRIMARY KEY,
                alias TEXT NOT NULL DEFAULT '',
                signature TEXT NOT NULL DEFAULT '',
                group_name TEXT NOT NULL DEFAULT ''
            )",
            [],
        )?;

        // Always ensure blacklist table exists (for existing DBs without it)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS blacklist (
                ip TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

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

    // ─── Contact Meta (alias, signature, group) ───────────────

    /// Get contact meta for an IP: (alias, signature, group_name)
    pub fn get_contact_meta(&self, ip: &str) -> anyhow::Result<Option<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT alias, signature, group_name FROM contact_meta WHERE ip = ?1",
        )?;
        let mut rows = stmt.query_map(params![ip], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        match rows.next() {
            Some(Ok(meta)) => Ok(Some(meta)),
            _ => Ok(None),
        }
    }

    /// Set (or update) the alias for a contact IP
    pub fn set_contact_alias(&self, ip: &str, alias: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO contact_meta (ip, alias, signature, group_name)
             VALUES (?1, ?2, '', '')
             ON CONFLICT(ip) DO UPDATE SET alias = excluded.alias",
            params![ip, alias],
        )?;
        Ok(())
    }

    /// Set (or update) the group name for a contact IP
    pub fn set_contact_group(&self, ip: &str, group_name: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO contact_meta (ip, alias, signature, group_name)
             VALUES (?1, '', '', ?2)
             ON CONFLICT(ip) DO UPDATE SET group_name = excluded.group_name",
            params![ip, group_name],
        )?;
        Ok(())
    }

    /// Load all contact meta as a map: IP -> (alias, signature, group_name)
    pub fn load_all_contact_meta(&self) -> anyhow::Result<HashMap<String, (String, String, String)>> {
        let mut stmt =
            self.conn
                .prepare("SELECT ip, alias, signature, group_name FROM contact_meta")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut result = HashMap::new();
        for row in rows {
            let (ip, alias, signature, group_name) = row?;
            result.insert(ip, (alias, signature, group_name));
        }
        Ok(result)
    }

    // ─── Blacklist ──────────────────────────────────────────────

    /// Check whether an IP is blacklisted
    pub fn is_blacklisted(&self, ip: &str) -> anyhow::Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM blacklist WHERE ip = ?1",
            params![ip],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Add an IP to the blacklist (ignores duplicates)
    pub fn add_to_blacklist(&self, ip: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO blacklist (ip, created_at) VALUES (?1, ?2)",
            params![ip, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    /// Remove an IP from the blacklist
    pub fn remove_from_blacklist(&self, ip: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM blacklist WHERE ip = ?1",
            params![ip],
        )?;
        Ok(())
    }

    /// Get all blacklisted IPs (ordered by creation time)
    pub fn get_blacklist(&self) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT ip FROM blacklist ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ─── Export / Import ────────────────────────────────────────

    /// Export all messages as a JSON structure
    pub fn export_all(&self) -> anyhow::Result<serde_json::Value> {
        let mut stmt = self.conn.prepare(
            "SELECT id, contact_ip, contact_name, direction, content_json, created_at
             FROM messages ORDER BY created_at",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "contact_ip": row.get::<_, String>(1)?,
                "contact_name": row.get::<_, String>(2)?,
                "direction": row.get::<_, i32>(3)?,
                "content_json": row.get::<_, String>(4)?,
                "created_at": row.get::<_, i64>(5)?,
            }))
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }

        Ok(serde_json::json!({
            "version": 1,
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "messages": messages,
        }))
    }

    /// Import messages from a JSON array, skipping duplicates.
    /// Duplicates are identified by matching (created_at, contact_ip, content_json).
    pub fn import_messages(&self, messages: &[serde_json::Value]) -> anyhow::Result<usize> {
        let mut count = 0;
        for msg in messages {
            let contact_ip = msg["contact_ip"].as_str().unwrap_or("");
            let content_json = msg["content_json"].as_str().unwrap_or("");
            let created_at = msg["created_at"].as_i64().unwrap_or(0);
            let contact_name = msg["contact_name"].as_str().unwrap_or("");
            let direction = msg["direction"].as_i64().unwrap_or(0) as i32;

            if contact_ip.is_empty() {
                continue;
            }

            // Check for duplicate by created_at + contact_ip + content_json
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM messages
                     WHERE created_at = ?1 AND contact_ip = ?2 AND content_json = ?3",
                    params![created_at, contact_ip, content_json],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

            if !exists {
                self.conn.execute(
                    "INSERT INTO messages (contact_ip, contact_name, direction, content_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![contact_ip, contact_name, direction, content_json, created_at],
                )?;
                count += 1;
            }
        }
        Ok(count)
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

        let _ = std::fs::remove_file(&path);
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

        let _ = std::fs::remove_file(&path);
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

    #[test]
    fn test_group_rename_no_duplicates() {
        let path = format!("/tmp/test_feix_grp_ren_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Initial save
        db.save_group("Dev Team", &["10.0.0.1".into(), "10.0.0.2".into()])
            .unwrap();

        // Rename by updating members (same group name, different members)
        // This should DELETE old entry and INSERT new one
        db.save_group("Dev Team", &["10.0.0.1".into(), "10.0.0.2".into(), "10.0.0.3".into()])
            .unwrap();

        let groups = db.get_groups().unwrap();
        // Should have exactly one entry for "Dev Team"
        let dev_teams: Vec<_> = groups.iter().filter(|(name, _)| name == "Dev Team").collect();
        assert_eq!(dev_teams.len(), 1, "Should not have duplicate groups after rename");
        // Should have 3 members
        assert_eq!(dev_teams[0].1.len(), 3);
        assert!(dev_teams[0].1.contains(&"10.0.0.3".to_string()));

        // Save same group again with same data — still only one entry
        db.save_group("Dev Team", &["10.0.0.1".into(), "10.0.0.2".into(), "10.0.0.3".into()])
            .unwrap();
        let groups = db.get_groups().unwrap();
        let dev_teams: Vec<_> = groups.iter().filter(|(name, _)| name == "Dev Team").collect();
        assert_eq!(dev_teams.len(), 1, "Re-saving same group should not create duplicates");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_contact_meta_crud() {
        let path = format!("/tmp/test_feix_meta_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Initially no meta
        let meta = db.get_contact_meta("10.0.0.1").unwrap();
        assert!(meta.is_none());

        // Set alias
        db.set_contact_alias("10.0.0.1", "Alice").unwrap();
        let meta = db.get_contact_meta("10.0.0.1").unwrap().unwrap();
        assert_eq!(meta.0, "Alice");
        assert_eq!(meta.1, ""); // signature
        assert_eq!(meta.2, ""); // group_name

        // Update alias
        db.set_contact_alias("10.0.0.1", "Alice2").unwrap();
        let meta = db.get_contact_meta("10.0.0.1").unwrap().unwrap();
        assert_eq!(meta.0, "Alice2");

        // Set group
        db.set_contact_group("10.0.0.1", "Dev Team").unwrap();
        let meta = db.get_contact_meta("10.0.0.1").unwrap().unwrap();
        assert_eq!(meta.2, "Dev Team");
        assert_eq!(meta.0, "Alice2"); // alias unchanged

        // Set another contact
        db.set_contact_alias("10.0.0.2", "Bob").unwrap();

        // Load all
        let all = db.load_all_contact_meta().unwrap();
        assert!(all.contains_key("10.0.0.1"));
        assert!(all.contains_key("10.0.0.2"));
        assert_eq!(all.get("10.0.0.1").unwrap().0, "Alice2");
        assert_eq!(all.get("10.0.0.1").unwrap().2, "Dev Team");
        assert_eq!(all.get("10.0.0.2").unwrap().0, "Bob");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_import_roundtrip() {
        let path = format!("/tmp/test_feix_exp_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Add some messages
        let content_a = vec![Content::Text {
            text: "Hello".into(),
            format: String::new(),
        }];
        db.save_message("10.0.0.1", "Alice", 0, &content_a).unwrap();

        let content_b = vec![Content::Text {
            text: "World".into(),
            format: String::new(),
        }];
        db.save_message("10.0.0.1", "Alice", 1, &content_b).unwrap();

        // Export
        let exported = db.export_all().unwrap();
        assert_eq!(exported["version"], 1);
        let messages = exported["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);

        // Import to a fresh DB
        let path2 = format!("/tmp/test_feix_exp2_{}.sqlite3", std::process::id());
        let db2 = HistoryDb::open(Path::new(&path2)).unwrap();

        let count = db2.import_messages(messages).unwrap();
        assert_eq!(count, 2);

        // Verify imported messages
        let imported = db2.get_messages("10.0.0.1", 0, 10).unwrap();
        assert_eq!(imported.len(), 2);

        // Import again — duplicates should be skipped
        let count2 = db2.import_messages(messages).unwrap();
        assert_eq!(count2, 0);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&path2);
    }

    #[test]
    fn test_import_skips_invalid() {
        let path = format!("/tmp/test_feix_imp_inv_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Empty contact_ip should be skipped
        let msgs = vec![serde_json::json!({
            "contact_ip": "",
            "content_json": "[]",
            "created_at": 1000,
            "contact_name": "",
            "direction": 0,
        })];
        let count = db.import_messages(&msgs).unwrap();
        assert_eq!(count, 0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_blacklist_crud() {
        let path = format!("/tmp/test_feix_bl_{}.sqlite3", std::process::id());
        let db = HistoryDb::open(Path::new(&path)).unwrap();

        // Initially no IP is blacklisted
        assert!(!db.is_blacklisted("10.0.0.1").unwrap());
        assert!(!db.is_blacklisted("10.0.0.2").unwrap());

        // Add to blacklist
        db.add_to_blacklist("10.0.0.1").unwrap();
        assert!(db.is_blacklisted("10.0.0.1").unwrap());
        assert!(!db.is_blacklisted("10.0.0.2").unwrap());

        // Get blacklist returns the single entry
        let list = db.get_blacklist().unwrap();
        assert_eq!(list, vec!["10.0.0.1"]);

        // Remove from blacklist
        db.remove_from_blacklist("10.0.0.1").unwrap();
        assert!(!db.is_blacklisted("10.0.0.1").unwrap());

        // Add multiple
        db.add_to_blacklist("10.0.0.1").unwrap();
        db.add_to_blacklist("10.0.0.2").unwrap();
        db.add_to_blacklist("10.0.0.3").unwrap();
        let list = db.get_blacklist().unwrap();
        assert_eq!(list.len(), 3);
        // Order should match insertion order
        assert_eq!(list[0], "10.0.0.1");
        assert_eq!(list[1], "10.0.0.2");
        assert_eq!(list[2], "10.0.0.3");

        // Duplicate add is silently ignored
        db.add_to_blacklist("10.0.0.1").unwrap();
        let list = db.get_blacklist().unwrap();
        assert_eq!(list.len(), 3, "Duplicate insert should not add a second entry");

        // Remove non-existent IP does not error
        db.remove_from_blacklist("999.999.999.999").unwrap();

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
}
