//! Upstream response fixtures, looked up by identity in the index the
//! extractor writes beside them under `tests/fixtures/upstream/alpaca-py/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One recorded upstream response.
#[derive(Debug)]
pub struct Fixture {
    pub status: u16,
    /// The headers the upstream test sets, and no others (a test that needs a
    /// `Content-Type` sets it itself), in name order rather than the index's
    /// source order: order between distinct header names carries no meaning.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

struct Entry {
    file: String,
    status: u16,
    headers: Vec<(String, String)>,
}

fn upstream_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream/alpaca-py")
}

fn index() -> &'static HashMap<String, Entry> {
    static INDEX: OnceLock<HashMap<String, Entry>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let path = upstream_dir().join("index.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let index: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()));
        let mut entries = HashMap::new();
        for f in index["fixtures"]
            .as_array()
            .expect("index has a fixtures array")
        {
            let id = f["id"].as_str().expect("fixture id is a string");
            let status = f["status"]
                .as_u64()
                .and_then(|s| u16::try_from(s).ok())
                .unwrap_or_else(|| panic!("{id}: status is not an HTTP status"));
            let headers = f["headers"]
                .as_object()
                .unwrap_or_else(|| panic!("{id}: headers is not an object"))
                .iter()
                .map(|(name, value)| {
                    let value = value
                        .as_str()
                        .unwrap_or_else(|| panic!("{id}: header {name} is not a string"));
                    (name.clone(), value.to_owned())
                })
                .collect();
            let file = f["file"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: file is not a string"))
                .to_owned();
            let entry = Entry {
                file,
                status,
                headers,
            };
            assert!(
                entries.insert(id.to_owned(), entry).is_none(),
                "duplicate fixture identity {id}"
            );
        }
        entries
    })
}

/// The fixture with this identity. An unknown identity or an unreadable file
/// panics: a missing fixture fails the test's setup, it never skips the test.
pub fn upstream(id: &str) -> Fixture {
    let entry = index()
        .get(id)
        .unwrap_or_else(|| panic!("unknown upstream fixture identity: {id}"));
    let path = upstream_dir().join(&entry.file);
    let body =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    Fixture {
        status: entry.status,
        headers: entry.headers.clone(),
        body,
    }
}
