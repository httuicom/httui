//! Session-scoped connection host:port overrides. In-memory only —
//! TEMPORARY by definition: an override wins over the stored
//! host/port for the session and disappears on restart. Never
//! persisted; the base connection is never mutated.

use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectionOverride {
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl ConnectionOverride {
    pub fn is_empty(&self) -> bool {
        self.host.is_none() && self.port.is_none()
    }
}

#[derive(Debug, Default)]
pub struct ConnectionOverrideStore {
    by_id: HashMap<String, ConnectionOverride>,
}

impl ConnectionOverrideStore {
    pub fn get(&self, connection_id: &str) -> Option<&ConnectionOverride> {
        self.by_id.get(connection_id)
    }

    /// Empty patch (host AND port unset) drops the entry.
    pub fn set(&mut self, connection_id: &str, ov: ConnectionOverride) {
        if ov.is_empty() {
            self.by_id.remove(connection_id);
        } else {
            self.by_id.insert(connection_id.to_string(), ov);
        }
    }

    pub fn clear(&mut self, connection_id: &str) {
        self.by_id.remove(connection_id);
    }

    pub fn is_active(&self, connection_id: &str) -> bool {
        self.by_id.contains_key(connection_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ov(host: Option<&str>, port: Option<u16>) -> ConnectionOverride {
        ConnectionOverride {
            host: host.map(|s| s.to_string()),
            port,
        }
    }

    #[test]
    fn is_empty_when_both_unset() {
        assert!(ov(None, None).is_empty());
        assert!(!ov(Some("h"), None).is_empty());
        assert!(!ov(None, Some(5432)).is_empty());
    }

    #[test]
    fn set_then_get_roundtrips() {
        let mut store = ConnectionOverrideStore::default();
        store.set("pg", ov(Some("db.staging"), Some(15432)));
        assert_eq!(store.get("pg"), Some(&ov(Some("db.staging"), Some(15432))));
    }

    #[test]
    fn set_empty_drops_entry() {
        let mut store = ConnectionOverrideStore::default();
        store.set("pg", ov(Some("db.staging"), Some(15432)));
        assert!(store.is_active("pg"));
        store.set("pg", ov(None, None));
        assert!(!store.is_active("pg"));
        assert_eq!(store.get("pg"), None);
    }

    #[test]
    fn clear_removes_entry() {
        let mut store = ConnectionOverrideStore::default();
        store.set("pg", ov(Some("h"), Some(1)));
        store.clear("pg");
        assert!(!store.is_active("pg"));
    }

    #[test]
    fn clear_unknown_is_noop() {
        let mut store = ConnectionOverrideStore::default();
        store.clear("nope");
        assert!(!store.is_active("nope"));
    }

    #[test]
    fn other_connections_unaffected() {
        let mut store = ConnectionOverrideStore::default();
        store.set("pg", ov(Some("h1"), None));
        store.set("mysql", ov(None, Some(13306)));
        assert!(store.is_active("pg"));
        assert!(store.is_active("mysql"));
        store.clear("pg");
        assert!(!store.is_active("pg"));
        assert!(store.is_active("mysql"));
    }
}
