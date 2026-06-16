pub mod store;

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub id: String,
    pub key_hash: String,
    pub tier: String,
    pub label: String,
    pub is_revoked: bool,
    pub created_at: i64,
}

pub trait KeyStore: Send + Sync {
    fn lookup(&self, raw_key: &str) -> Result<Option<KeyEntry>, anyhow::Error>;
    fn create_key(&self, raw_key: &str, tier: &str, label: &str) -> Result<KeyEntry, anyhow::Error>;
    fn revoke_key(&self, key_id: &str) -> Result<bool, anyhow::Error>;
    fn list_keys(&self) -> Result<Vec<KeyEntry>, anyhow::Error>;
}

pub enum KeyStoreDispatch {
    Sqlite(Arc<store::sqlite::SqliteKeyStore>),
}

impl KeyStore for KeyStoreDispatch {
    fn lookup(&self, raw_key: &str) -> Result<Option<KeyEntry>, anyhow::Error> {
        match self {
            KeyStoreDispatch::Sqlite(store) => store.lookup(raw_key),
        }
    }

    fn create_key(&self, raw_key: &str, tier: &str, label: &str) -> Result<KeyEntry, anyhow::Error> {
        match self {
            KeyStoreDispatch::Sqlite(store) => store.create_key(raw_key, tier, label),
        }
    }

    fn revoke_key(&self, key_id: &str) -> Result<bool, anyhow::Error> {
        match self {
            KeyStoreDispatch::Sqlite(store) => store.revoke_key(key_id),
        }
    }

    fn list_keys(&self) -> Result<Vec<KeyEntry>, anyhow::Error> {
        match self {
            KeyStoreDispatch::Sqlite(store) => store.list_keys(),
        }
    }
}

pub fn extract_api_key(headers: &axum::http::HeaderMap, uri: &axum::http::Uri) -> Option<String> {
    if let Some(auth) = headers.get("Authorization") {
        let val = auth.to_str().ok()?;
        if let Some(key) = val.strip_prefix("Bearer ") {
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }

    if let Some(key) = headers.get("X-API-Key") {
        let val = key.to_str().ok()?;
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }

    if let Some(query) = uri.query() {
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            if k == "api_key" && !v.is_empty() {
                return Some(v.into_owned());
            }
        }
    }

    None
}

#[derive(Clone)]
pub struct AuthTier(pub String);

#[derive(Clone)]
pub struct AuthKeyId(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Uri};

    #[test]
    fn test_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer sk_test_abc123"));
        let uri = Uri::from_static("/");
        assert_eq!(extract_api_key(&headers, &uri), Some("sk_test_abc123".to_string()));
    }

    #[test]
    fn test_extract_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("sk_test_xyz789"));
        let uri = Uri::from_static("/");
        assert_eq!(extract_api_key(&headers, &uri), Some("sk_test_xyz789".to_string()));
    }

    #[test]
    fn test_extract_query_param() {
        let headers = HeaderMap::new();
        let uri = Uri::from_static("/?api_key=sk_test_query");
        assert_eq!(extract_api_key(&headers, &uri), Some("sk_test_query".to_string()));
    }

    #[test]
    fn test_bearer_takes_precedence() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer sk_bearer"));
        headers.insert("X-API-Key", HeaderValue::from_static("sk_header"));
        let uri = Uri::from_static("/?api_key=sk_query");
        assert_eq!(extract_api_key(&headers, &uri), Some("sk_bearer".to_string()));
    }

    #[test]
    fn test_no_key_returns_none() {
        let headers = HeaderMap::new();
        let uri = Uri::from_static("/");
        assert_eq!(extract_api_key(&headers, &uri), None);
    }

    #[test]
    fn test_empty_bearer_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer "));
        let uri = Uri::from_static("/");
        assert_eq!(extract_api_key(&headers, &uri), None);
    }

    #[test]
    fn test_wrong_auth_scheme_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Basic dXNlcjpwYXNz"));
        let uri = Uri::from_static("/");
        assert_eq!(extract_api_key(&headers, &uri), None);
    }
}
