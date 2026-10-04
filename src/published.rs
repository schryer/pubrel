//! Published objects, read and checked in-process with `publet-core`.
//!
//! Reading the package publet back -- its version, its release history --
//! and checking a release is signed need only canonical CBOR, content
//! identifiers, and Ed25519, which is what `publet-core` is. So `check`,
//! `tag`, `next` and `changelog` run without `pub`; only `prepare`, which
//! signs, needs it.

use std::fmt::Write as _;
use std::path::PathBuf;

use publet_core::cbor::Value as Cbor;
use publet_core::{Cid, Object, SigAlg};
use serde_json::{Map, Value, json};

use crate::record::Config;

/// The purpose `pub` signs what it publishes for.
const PURPOSE: &str = "authored";

fn path_of(cfg: &Config, cid: &str) -> PathBuf {
    cfg.corpus_dir()
        .join("objects")
        .join(format!("{}.cbor", cid.replace(':', "_")))
}

/// The object `cid` names, from the corpus's objects, after checking the
/// bytes there are the bytes that identifier names.
fn load(cfg: &Config, cid: &str) -> Result<Object, String> {
    let expected: Cid = cid
        .parse()
        .map_err(|_| format!("{cid} is not an identifier"))?;
    let path = path_of(cfg, cid);
    let bytes = std::fs::read(&path).map_err(|_| {
        format!(
            "{cid} is not in {}",
            cfg.corpus_dir().join("objects").display()
        )
    })?;
    let object = Object::parse(&bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .verify(&expected)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(object.into_inner())
}

fn to_json(value: &Cbor) -> Value {
    match value {
        Cbor::Uint(n) => json!(n),
        Cbor::Nint(n) => json!(-1 - i128::from(*n)),
        Cbor::Bytes(b) => {
            let hex = b.iter().fold(String::new(), |mut out, x| {
                let _ = write!(out, "{x:02x}");
                out
            });
            json!({ "$bytes": hex })
        }
        Cbor::Text(t) => json!(t),
        Cbor::Array(items) => Value::Array(items.iter().map(to_json).collect()),
        Cbor::Map(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect::<Map<_, _>>(),
        ),
        Cbor::Bool(b) => json!(b),
        // Null, and any kind of value a later protocol adds.
        _ => Value::Null,
    }
}

/// A published object as JSON, in the shape `pub read --json` gives its
/// `object`: the header fields and the body, bytes as `{"$bytes": hex}`.
///
/// # Errors
///
/// Returns a message if it is not held, or its bytes are not the ones
/// `cid` names.
pub fn read_object(cfg: &Config, cid: &str) -> Result<Value, String> {
    let object = load(cfg, cid)?;
    let mut out = Map::new();
    out.insert("pub".into(), json!(object.protocol()));
    out.insert("type".into(), json!(object.kind()));
    out.insert("created".into(), json!(object.created()));
    out.insert("author".into(), json!(object.author().to_string()));
    out.insert(
        "body".into(),
        Value::Object(
            object
                .body()
                .iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect(),
        ),
    );
    if let Some(prev) = object.prev() {
        out.insert("prev".into(), json!(prev.to_string()));
    }
    if let Some(basis) = object.basis() {
        out.insert("basis".into(), json!(basis.to_string()));
    }
    Ok(Value::Object(out))
}

fn text<'a>(object: &'a Object, field: &str) -> Option<&'a str> {
    match object.body().get(field) {
        Some(Cbor::Text(t)) => Some(t),
        _ => None,
    }
}

fn bytes<'a>(object: &'a Object, field: &str) -> Option<&'a [u8]> {
    match object.body().get(field) {
        Some(Cbor::Bytes(b)) => Some(b),
        _ => None,
    }
}

/// Check `cid` carries a valid signature by the key `release.json` names.
///
/// The key is trusted because the repository names it: a change to it is a
/// change to `release.json`, reviewed like any other. Its public key is
/// read from its own `key` object, whose identifier is checked like every
/// other object's.
///
/// # Errors
///
/// Returns a message saying what is missing or does not verify.
pub fn require_signed(cfg: &Config, cid: &str) -> Result<(), String> {
    let Some(key_cid) = cfg.key.as_deref() else {
        eprintln!(
            "warning: release.json names no `key`, so {cid}'s signature is not checked; \
             add \"key\": the corpus's author (`author=` in {}/.publet/config)",
            cfg.corpus.display()
        );
        return Ok(());
    };
    let key = load(cfg, key_cid).map_err(|e| format!("the release key: {e}"))?;
    if key.kind() != "key" || text(&key, "alg") != Some("ed25519") {
        return Err(format!(
            "{key_cid}, which release.json names, is not an ed25519 key"
        ));
    }
    let public = bytes(&key, "pubkey").ok_or_else(|| format!("{key_cid} states no public key"))?;
    let target = load(cfg, cid)?;
    let dir = cfg.corpus_dir().join("objects");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".cbor") else {
            continue;
        };
        let Ok(sig) = load(cfg, &stem.replace('_', ":")) else {
            continue;
        };
        if sig.kind() != "sig"
            || sig.author().to_string() != key_cid
            || text(&sig, "target") != Some(cid)
        {
            continue;
        }
        let (Some(purpose), Some(value)) = (text(&sig, "purpose"), bytes(&sig, "value")) else {
            continue;
        };
        if publet_core::verify(
            SigAlg::Ed25519,
            public,
            value,
            purpose,
            PURPOSE,
            target.bytes(),
        )
        .is_ok()
        {
            return Ok(());
        }
    }
    Err(format!(
        "{cid} has no valid signature by {key_cid}, the key release.json names"
    ))
}
