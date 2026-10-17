//! The fixture tree under `tests/fixtures/` is exactly what its two manifests
//! list: the upstream `index.json` for `upstream/alpaca-py/`, and
//! `MANIFEST.toml` for every other file. A file owned by neither, a listed
//! file that is missing, and a size or SHA-256 mismatch each fail, so fixture
//! bytes can neither drift nor grow silently.

#[allow(dead_code)]
#[path = "support/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const UPSTREAM: &str = "upstream/alpaca-py";

/// Files no manifest lists: those the extractor writes beside the upstream
/// fixtures (and re-creates byte for byte on every check), and the manifest
/// itself, which is the reviewed source of truth.
const UNLISTED: [&str; 4] = [
    "upstream/alpaca-py/index.json",
    "upstream/alpaca-py/LICENSE",
    "upstream/alpaca-py/README.md",
    "MANIFEST.toml",
];

const KEYS: [&str; 5] = ["path", "sha256", "size", "class", "source"];

/// What a manifest says one file holds.
struct Listed {
    size: u64,
    sha256: String,
    owner: &'static str,
}

/// Parse `MANIFEST.toml`: comments, `[[file]]` tables and `key = "string"` or
/// `key = integer` lines, each table with exactly [`KEYS`]. Anything else is
/// an error.
fn parse_manifest(text: &str) -> Result<Vec<BTreeMap<String, String>>, String> {
    let mut entries: Vec<BTreeMap<String, String>> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let n = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[file]]" {
            entries.push(BTreeMap::new());
            continue;
        }
        let (key, value) = line
            .split_once(" = ")
            .ok_or_else(|| format!("line {n}: not `key = value`: {raw}"))?;
        let value = if let Some(s) = value.strip_prefix('"') {
            let s = s
                .strip_suffix('"')
                .ok_or_else(|| format!("line {n}: unterminated string"))?;
            if s.contains(['"', '\\']) {
                return Err(format!("line {n}: a quote or backslash inside a string"));
            }
            s.to_owned()
        } else if value.bytes().all(|b| b.is_ascii_digit()) {
            value.to_owned()
        } else {
            return Err(format!("line {n}: unsupported value {value}"));
        };
        if !KEYS.contains(&key) {
            return Err(format!("line {n}: unknown key {key}"));
        }
        let entry = entries
            .last_mut()
            .ok_or_else(|| format!("line {n}: {key} outside a [[file]] table"))?;
        if entry.insert(key.to_owned(), value).is_some() {
            return Err(format!("line {n}: duplicate key {key}"));
        }
    }
    for entry in &entries {
        for key in KEYS {
            if !entry.contains_key(key) {
                return Err(format!("entry {entry:?} lacks `{key}`"));
            }
        }
        if entry["class"] != "synthetic" {
            return Err(format!("entry {entry:?}: class must be \"synthetic\""));
        }
    }
    Ok(entries)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every file under `root`, relative and `/`-separated, and every hidden
/// directory (a staging directory an interrupted extraction left behind).
fn walk(root: &Path) -> (Vec<String>, Vec<String>) {
    fn go(root: &Path, dir: &Path, files: &mut Vec<String>, hidden: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if path.file_name().unwrap().to_string_lossy().starts_with('.') {
                    hidden.push(rel);
                } else {
                    go(root, &path, files, hidden);
                }
            } else {
                files.push(rel);
            }
        }
    }
    let (mut files, mut hidden) = (Vec::new(), Vec::new());
    go(root, root, &mut files, &mut hidden);
    files.sort();
    hidden.sort();
    (files, hidden)
}

fn upstream_listing(root: &Path) -> Result<Vec<(String, Listed)>, String> {
    let path = root.join(UPSTREAM).join("index.json");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read index.json: {e}"))?;
    let index: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("index.json is not JSON: {e}"))?;
    let fixtures = index["fixtures"]
        .as_array()
        .ok_or("index.json has no fixtures array")?;
    let mut out = Vec::new();
    for f in fixtures {
        let (Some(file), Some(size), Some(sha256)) = (
            f["file"].as_str(),
            f["bytes"].as_u64(),
            f["sha256"].as_str(),
        ) else {
            return Err(format!(
                "index entry lacks file, bytes or sha256: {}",
                f["id"]
            ));
        };
        if !confined(file) {
            return Err(format!(
                "index.json: {file} is not a path inside its directory"
            ));
        }
        let listed = Listed {
            size,
            sha256: sha256.to_owned(),
            owner: "index.json",
        };
        out.push((format!("{UPSTREAM}/{file}"), listed));
    }
    Ok(out)
}

/// A relative path that stays inside its directory: no leading `/`, and no
/// empty, `.` or `..` component.
fn confined(path: &str) -> bool {
    !path.starts_with('/')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn manifest_listing(root: &Path) -> Result<Vec<(String, Listed)>, String> {
    let text = std::fs::read_to_string(root.join("MANIFEST.toml"))
        .map_err(|e| format!("cannot read MANIFEST.toml: {e}"))?;
    let mut out = Vec::new();
    for entry in parse_manifest(&text)? {
        let path = &entry["path"];
        if !confined(path) || path.starts_with(&format!("{UPSTREAM}/")) {
            return Err(format!(
                "MANIFEST.toml: {path} is not a path under tests/fixtures/ outside {UPSTREAM}/"
            ));
        }
        let size = entry["size"]
            .parse()
            .map_err(|e| format!("{}: size: {e}", entry["path"]))?;
        let listed = Listed {
            size,
            sha256: entry["sha256"].clone(),
            owner: "MANIFEST.toml",
        };
        out.push((entry["path"].clone(), listed));
    }
    Ok(out)
}

/// Check the fixture tree at `root` against both manifests. Returns the number
/// of files they list, or every problem found.
fn verify(root: &Path) -> Result<usize, Vec<String>> {
    let mut problems = Vec::new();
    let mut listed: BTreeMap<String, Listed> = BTreeMap::new();
    for listing in [upstream_listing(root), manifest_listing(root)] {
        match listing {
            Ok(entries) => {
                for (path, entry) in entries {
                    if let Some(first) = listed.get(&path) {
                        problems.push(format!(
                            "{path} is listed twice ({} and {})",
                            first.owner, entry.owner
                        ));
                    }
                    listed.insert(path, entry);
                }
            }
            Err(e) => problems.push(e),
        }
    }
    for (path, entry) in &listed {
        match std::fs::read(root.join(path)) {
            Ok(bytes) => {
                if bytes.len() as u64 != entry.size {
                    problems.push(format!(
                        "{path}: size {} differs from {} in {}",
                        bytes.len(),
                        entry.size,
                        entry.owner
                    ));
                }
                if sha256_hex(&bytes) != entry.sha256 {
                    problems.push(format!("{path}: sha256 differs from {}", entry.owner));
                }
            }
            Err(e) => problems.push(format!("{path}, listed in {}: {e}", entry.owner)),
        }
    }
    let (files, hidden) = walk(root);
    for path in files {
        if !listed.contains_key(&path) && !UNLISTED.contains(&path.as_str()) {
            problems.push(format!("{path} is on disk but no manifest lists it"));
        }
    }
    for dir in hidden {
        problems.push(format!(
            "{dir}: hidden directory, left by an interrupted extraction"
        ));
    }
    if problems.is_empty() {
        Ok(listed.len())
    } else {
        Err(problems)
    }
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// A scratch copy of the fixture tree, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dst = std::env::temp_dir().join(format!("suri-fixtures-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        let src = fixtures_dir();
        let (files, hidden) = walk(&src);
        assert!(hidden.is_empty(), "{hidden:?}");
        for rel in files {
            let to = dst.join(&rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(src.join(&rel), to).unwrap();
        }
        Scratch(dst)
    }

    fn problems(&self) -> Vec<String> {
        verify(&self.0).unwrap_err()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TEXT_FILE: &str = "upstream/alpaca-py/broker/broker_client/test_documents_routes/test_download_trade_document_for_account_by_id/get-v1-accounts-account-id-documents-document-id-download-0.txt";

fn any(problems: &[String], needle: &str) -> bool {
    problems.iter().any(|p| p.contains(needle))
}

#[test]
fn the_committed_tree_matches_its_manifests() {
    let listed = verify(&fixtures_dir()).unwrap_or_else(|p| panic!("{}", p.join("\n")));
    // 191 upstream fixtures; MANIFEST.toml lists none yet.
    assert_eq!(listed, 191);
}

#[test]
fn a_file_no_manifest_lists_fails() {
    let copy = Scratch::new("unowned");
    std::fs::write(copy.0.join("upstream/alpaca-py/extra.json"), b"{}").unwrap();
    std::fs::write(copy.0.join("stray.json"), b"{}").unwrap();
    let problems = copy.problems();
    assert!(
        any(
            &problems,
            "upstream/alpaca-py/extra.json is on disk but no manifest lists it"
        ),
        "{problems:?}"
    );
    assert!(any(&problems, "stray.json is on disk"), "{problems:?}");
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn a_missing_listed_file_fails() {
    let copy = Scratch::new("missing");
    std::fs::remove_file(copy.0.join(TEXT_FILE)).unwrap();
    let problems = copy.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with(&format!("{TEXT_FILE}, listed in index.json: ")));
}

#[test]
fn a_changed_byte_fails() {
    let copy = Scratch::new("byte");
    let path = copy.0.join(TEXT_FILE);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[10] ^= 0x01;
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        copy.problems(),
        [format!("{TEXT_FILE}: sha256 differs from index.json")]
    );
}

#[test]
fn a_size_mismatch_fails() {
    let copy = Scratch::new("size");
    let path = copy.0.join(TEXT_FILE);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();
    let problems = copy.problems();
    assert!(
        any(
            &problems,
            &format!("{TEXT_FILE}: size 128 differs from 127 in index.json")
        ),
        "{problems:?}"
    );
}

#[test]
fn a_leftover_staging_directory_fails() {
    let copy = Scratch::new("leftover");
    std::fs::create_dir_all(copy.0.join("upstream/.alpaca-py.new")).unwrap();
    assert_eq!(
        copy.problems(),
        ["upstream/.alpaca-py.new: hidden directory, left by an interrupted extraction"]
    );
}

#[test]
fn a_manifest_line_outside_the_subset_fails() {
    for (text, expect) in [
        ("[file]\n", "line 1: not `key = value`"),
        ("[[file]]\npath = 'a.json'\n", "line 2: unsupported value"),
        ("[[file]]\nsize = 1.5\n", "line 2: unsupported value"),
        ("[[file]]\nsize = -1\n", "line 2: unsupported value"),
        (
            "path = \"a.json\"\n",
            "line 1: path outside a [[file]] table",
        ),
        ("[[file]]\nowner = \"x\"\n", "line 2: unknown key owner"),
        (
            "[[file]]\npath = \"a\"\npath = \"b\"\n",
            "line 3: duplicate key path",
        ),
        (
            "[[file]]\npath = \"a\\\"b\"\n",
            "line 2: a quote or backslash",
        ),
        ("[[file]]\npath = \"a.json\"\n", "lacks `sha256`"),
        ("[[file]]\npath = \"a.json\n", "line 2: unterminated string"),
    ] {
        let copy = Scratch::new("subset");
        std::fs::write(copy.0.join("MANIFEST.toml"), text).unwrap();
        let problems = copy.problems();
        assert!(any(&problems, expect), "{text:?}: {problems:?}");
    }
}

#[test]
fn a_synthetic_entry_is_checked_and_only_that_class_is_accepted() {
    let copy = Scratch::new("synthetic");
    let body = b"{\"id\": \"synthetic\"}\n";
    std::fs::create_dir_all(copy.0.join("trading")).unwrap();
    std::fs::write(copy.0.join("trading/order.json"), body).unwrap();
    let entry = |path: &str, class: &str| {
        format!(
            "# one entry\n[[file]]\npath = \"{path}\"\nsha256 = \"{}\"\nsize = {}\nclass = \"{class}\"\nsource = \"alpaca:us/reference/postorder.md:1-2\"\n",
            sha256_hex(body),
            body.len()
        )
    };
    let manifest = copy.0.join("MANIFEST.toml");
    std::fs::write(&manifest, entry("trading/order.json", "synthetic")).unwrap();
    assert_eq!(verify(&copy.0), Ok(192));
    std::fs::write(&manifest, entry("trading/order.json", "upstream")).unwrap();
    assert!(any(&copy.problems(), "class must be \"synthetic\""));
    let twice = entry("trading/order.json", "synthetic").repeat(2);
    std::fs::write(&manifest, twice).unwrap();
    let problems = copy.problems();
    assert!(
        any(
            &problems,
            "trading/order.json is listed twice (MANIFEST.toml and MANIFEST.toml)"
        ),
        "{problems:?}"
    );
    // The upstream directory belongs to its index alone, and nothing outside
    // tests/fixtures/ can be listed.
    std::fs::write(copy.0.join("upstream/alpaca-py/order.json"), body).unwrap();
    for path in [
        "upstream/alpaca-py/order.json",
        "../order.json",
        "trading//order.json",
    ] {
        std::fs::write(&manifest, entry(path, "synthetic")).unwrap();
        let problems = copy.problems();
        assert!(
            any(
                &problems,
                &format!("MANIFEST.toml: {path} is not a path under tests/fixtures/")
            ),
            "{path}: {problems:?}"
        );
    }
}

#[test]
fn an_index_path_outside_its_directory_fails() {
    let copy = Scratch::new("index-path");
    let index = copy.0.join("upstream/alpaca-py/index.json");
    let text = std::fs::read_to_string(&index).unwrap();
    let file = TEXT_FILE.strip_prefix("upstream/alpaca-py/").unwrap();
    assert_eq!(text.matches(file).count(), 1);
    std::fs::write(&index, text.replace(file, "../../stray.txt")).unwrap();
    assert!(any(
        &copy.problems(),
        "index.json: ../../stray.txt is not a path inside its directory"
    ));
}

#[test]
fn the_loader_returns_status_headers_and_exact_bytes() {
    let f = fixture::upstream(
        "broker/broker_client/test_documents_routes::test_download_trade_document_for_account_by_id::GET /v1/accounts/{account_id}/documents/{document_id}/download#0",
    );
    assert_eq!(f.status, 301);
    let location = "https://fake-dl.com/2a87c089-ffb6-472b-a4a3-cd9305c8605d";
    assert_eq!(f.headers, [("Location".to_owned(), location.to_owned())]);
    // The f-string at alpaca-py tests/broker/broker_client/test_documents_routes.py:243-247.
    let body = format!(
        "\n        <a href=\"{location}\">\n          Moved Permanently\n        </a>.\n        "
    );
    assert_eq!(f.body, body.as_bytes());

    let empty = fixture::upstream(
        "broker/broker_client/test_accounts_routes::test_close_account::POST /v1/accounts/{account_id}/actions/close#0",
    );
    assert_eq!(empty.status, 204);
    assert!(empty.headers.is_empty());
    assert!(empty.body.is_empty());
}

#[test]
#[should_panic(
    expected = "unknown upstream fixture identity: trading/none::test_none::GET /v2/none#0"
)]
fn the_loader_panics_on_an_unknown_identity() {
    fixture::upstream("trading/none::test_none::GET /v2/none#0");
}
