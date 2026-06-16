use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

use crate::auth::{KeyEntry, KeyStore};

pub struct SqliteKeyStore {
    conn: Mutex<Connection>,
}

impl SqliteKeyStore {
    pub fn new(db_path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                key_hash TEXT NOT NULL,
                tier TEXT NOT NULL,
                label TEXT NOT NULL,
                is_revoked BOOLEAN NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn sha256_id(raw_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    fn hash_key(raw_key: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(raw_key.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Argon2 hashing failed: {}", e))?;
        Ok(hash.to_string())
    }
}

impl KeyStore for SqliteKeyStore {
    fn lookup(&self, raw_key: &str) -> Result<Option<KeyEntry>, anyhow::Error> {
        let key_id = Self::sha256_id(raw_key);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, key_hash, tier, label, is_revoked, created_at FROM api_keys WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![key_id], |row| {
            Ok(KeyEntry {
                id: row.get(0)?,
                key_hash: row.get(1)?,
                tier: row.get(2)?,
                label: row.get(3)?,
                is_revoked: row.get::<_, bool>(4)?,
                created_at: row.get(5)?,
            })
        })?;

        if let Some(result) = rows.next() {
            let entry = result?;
            let parsed_hash = PasswordHash::new(&entry.key_hash)
                .map_err(|e| anyhow::anyhow!("Invalid password hash: {}", e))?;
            if Argon2::default()
                .verify_password(raw_key.as_bytes(), &parsed_hash)
                .is_ok()
            {
                Ok(Some(entry))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn create_key(&self, raw_key: &str, tier: &str, label: &str) -> Result<KeyEntry, anyhow::Error> {
        let key_id = Self::sha256_id(raw_key);
        let key_hash = Self::hash_key(raw_key)?;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, key_hash, tier, label, is_revoked, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![key_id, key_hash, tier, label, created_at],
        )?;

        Ok(KeyEntry {
            id: key_id,
            key_hash,
            tier: tier.to_string(),
            label: label.to_string(),
            is_revoked: false,
            created_at,
        })
    }

    fn revoke_key(&self, key_id: &str) -> Result<bool, anyhow::Error> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE api_keys SET is_revoked = 1 WHERE id = ?1",
            params![key_id],
        )?;
        Ok(affected > 0)
    }

    fn list_keys(&self) -> Result<Vec<KeyEntry>, anyhow::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, key_hash, tier, label, is_revoked, created_at FROM api_keys ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(KeyEntry {
                id: row.get(0)?,
                key_hash: row.get(1)?,
                tier: row.get(2)?,
                label: row.get(3)?,
                is_revoked: row.get::<_, bool>(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> SqliteKeyStore {
        SqliteKeyStore::new(":memory:").unwrap()
    }

    #[test]
    fn test_create_and_lookup() {
        let store = setup_db();
        let key = "sk_test_create_lookup";

        store.create_key(key, "premium", "test key").unwrap();
        let result = store.lookup(key).unwrap();
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tier, "premium");
        assert_eq!(entry.label, "test key");
        assert!(!entry.is_revoked);
    }

    #[test]
    fn test_lookup_nonexistent() {
        let store = setup_db();
        let result = store.lookup("sk_nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_revoked_key_rejected() {
        let store = setup_db();
        let key = "sk_test_revoked";
        store.create_key(key, "basic", "to revoke").unwrap();
        let key_id = SqliteKeyStore::sha256_id(key);

        store.revoke_key(&key_id).unwrap();
        let lookup_result = store.lookup(key).unwrap();
        assert!(lookup_result.is_some());
        assert!(lookup_result.unwrap().is_revoked);
    }

    #[test]
    fn test_list_keys() {
        let store = setup_db();
        store.create_key("sk_test_list_1", "tier1", "key 1").unwrap();
        store.create_key("sk_test_list_2", "tier2", "key 2").unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
    }
}
