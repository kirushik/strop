use std::collections::BTreeMap;

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;

use super::{Channel, UPDATER_PROTOCOL};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub product: String,
    pub updater_protocol: u32,
    pub version: String,
    pub pub_date: String,
    pub notes_url: String,
    pub targets: BTreeMap<String, Target>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// The runtime trust anchor is the repo's committed `minisign.pub`, baked
/// at compile time: a keyless binary cannot be produced by mis-set CI
/// state, and key rotation is a commit (hence a release), never a mutable
/// variable. A rotation bridge is extra key blocks appended to the file.
pub fn baked_keys() -> Vec<PublicKey> {
    include_str!("../../../../minisign.pub")
        .lines()
        .filter_map(|s| {
            let line = s.trim();
            (!line.is_empty() && !line.starts_with("untrusted comment"))
                .then(|| PublicKey::from_base64(line).ok()).flatten()
        })
        .collect()
}

pub fn verify_signed(bytes: &[u8], signatures: &[Vec<u8>], keys: &[PublicKey]) -> Result<(), String> {
    if keys.is_empty() { return Err("no update signing keys are baked into this build".into()); }
    let decoded: Vec<_> = signatures.iter().filter_map(|raw| {
        std::str::from_utf8(raw).ok().and_then(|s| Signature::decode(s).ok())
    }).collect();
    if keys.iter().any(|key| decoded.iter().any(|sig| key.verify(bytes, sig, false).is_ok())) {
        Ok(())
    } else {
        Err("manifest signature did not match any trusted key".into())
    }
}

/// `Ok(None)` is the healthy everyday outcome: a valid, verified manifest
/// that simply offers nothing newer than this build. Only genuine defects
/// (wrong product/protocol/signature-adjacent fields, replay, bad URL) are
/// `Err` — an up-to-date install must land in Idle, never in Failed.
pub fn parse_and_validate(
    bytes: &[u8], channel: Channel, highest_seen: Option<&Version>,
) -> Result<Option<(Manifest, Option<Target>, Version)>, String> {
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|e| format!("invalid update manifest: {e}"))?;
    if manifest.product != "strop" { return Err("manifest is for another product".into()); }
    if manifest.updater_protocol != UPDATER_PROTOCOL {
        return Err("unsupported updater protocol".into());
    }
    let version = Version::parse(&manifest.version)
        .map_err(|_| "manifest version is not valid semver".to_owned())?;
    if !valid_rfc3339(&manifest.pub_date) {
        return Err("manifest publication date is not valid RFC3339".into());
    }
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is semver");
    if version <= current { return Ok(None); }
    if highest_seen.is_some_and(|highest| &version < highest) {
        return Err("manifest version is below the highest version previously seen".into());
    }
    if !manifest.notes_url.starts_with("https://github.com/kirushik/strop/") {
        return Err("manifest notes URL is outside the Strop repository".into());
    }
    // Passive channels only ever ANNOUNCE a newer version — they never
    // touch an artifact, so a manifest that (correctly) carries only the
    // self-update targets must still validate for them. Requiring a
    // target here made every passive channel fail closed the moment a
    // newer release existed: the exact moment the notice matters.
    if !channel.self_updates() {
        return Ok(Some((manifest, None, version)));
    }
    let key = target_key(channel).ok_or_else(|| "this channel has no update target".to_owned())?;
    let target = manifest.targets.get(&key).cloned()
        .ok_or_else(|| format!("manifest has no target {key}"))?;
    if target.size > super::MAX_ARTIFACT_SIZE { return Err("update artifact exceeds size limit".into()); }
    if target.size == 0 { return Err("update artifact declares no size".into()); }
    if target.sha256.len() != 64 || !target.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("target SHA-256 is malformed".into());
    }
    super::fetch::validate_url(&target.url)?;
    Ok(Some((manifest, Some(target), version)))
}

fn valid_rfc3339(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T') || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    { return false; }
    let number = |range: std::ops::Range<usize>| value.get(range)
        .and_then(|s| s.parse::<u32>().ok());
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (number(0..4), number(5..7), number(8..10), number(11..13),
            number(14..16), number(17..19)) else { return false; };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month { 1|3|5|7|8|10|12 => 31, 4|6|9|11 => 30,
        2 => if leap { 29 } else { 28 }, _ => return false };
    if day == 0 || day > days || hour > 23 || minute > 59 || second > 60 { return false; }
    let mut end = 19;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) { end += 1; }
        if end == start { return false; }
    }
    if bytes.get(end) == Some(&b'Z') { return end + 1 == bytes.len(); }
    if !matches!(bytes.get(end), Some(b'+') | Some(b'-')) || end + 6 != bytes.len()
        || bytes.get(end + 3) != Some(&b':') { return false; }
    let offset_hour = value.get(end + 1..end + 3).and_then(|s| s.parse::<u32>().ok());
    let offset_minute = value.get(end + 4..end + 6).and_then(|s| s.parse::<u32>().ok());
    matches!((offset_hour, offset_minute), (Some(h), Some(m)) if h <= 23 && m <= 59)
}

fn target_key(channel: Channel) -> Option<String> {
    let (name, kind) = match channel {
        Channel::GithubWin => ("github-win", "exe"),
        Channel::GithubMac => ("github-mac", "app-tar"),
        Channel::GithubWinPortable => ("github-win-portable", "exe"),
        Channel::GithubLinux => ("github-linux", "exe"),
        _ => return None,
    };
    Some(format!("{name}/{}/{kind}", target_triple()))
}

fn target_triple() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    { "x86_64-pc-windows-msvc" }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    { "aarch64-apple-darwin" }
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    { "x86_64-unknown-linux-gnu" }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "windows"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
    )))]
    { "unsupported-target" }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use minisign::{KeyPair, sign};
    use serde_json::{Value, json};
    use super::*;

    fn valid() -> Value {
        json!({
            "product": "strop", "updater_protocol": 1,
            "version": "0.3.1", "pub_date": "2026-07-18T12:34:56+02:00",
            "notes_url": "https://github.com/kirushik/strop/releases/tag/v0.3.1",
            "targets": {
                format!("github-win/{}/exe", target_triple()): {
                    "url": "https://github.com/kirushik/strop/releases/download/v0.3.1/strop",
                    "sha256": "00".repeat(32), "size": 3
                }
            }
        })
    }

    // Target-shaped refusals only bind on a SELF-UPDATE channel now:
    // passive channels validate the signed envelope and stop before
    // targets (they announce, never fetch).
    fn refused(mut value: Value, edit: impl FnOnce(&mut Value)) {
        edit(&mut value);
        assert!(parse_and_validate(&serde_json::to_vec(&value).unwrap(),
            Channel::GithubWin, None).is_err());
    }

    #[test]
    fn manifest_refusal_matrix() {
        let value = valid();
        assert!(parse_and_validate(&serde_json::to_vec(&value).unwrap(),
            Channel::GithubWin, None).is_ok());
        // The passive contract, pinned: a manifest carrying only the
        // self-update targets — the shipping shape — must still read as
        // Available (target-free) for every announce-only channel, even
        // with an empty targets table. This exact gap once turned "0.3.2
        // exists" into "couldn't check for updates" on all of them.
        for channel in [Channel::GithubLinux, Channel::GithubWinPortable,
            Channel::Deb, Channel::Rpm, Channel::Flathub]
        {
            assert!(matches!(
                parse_and_validate(&serde_json::to_vec(&valid()).unwrap(), channel, None),
                Ok(Some((_, None, _)))), "{channel:?}");
            let mut empty = valid();
            empty["targets"] = json!({});
            assert!(matches!(
                parse_and_validate(&serde_json::to_vec(&empty).unwrap(), channel, None),
                Ok(Some((_, None, _)))), "{channel:?}");
        }
        refused(valid(), |v| v["product"] = json!("other"));
        refused(valid(), |v| v["updater_protocol"] = json!(2));
        refused(valid(), |v| v["version"] = json!("banana"));
        // Same-or-older version is NOT a refusal — it is the healthy
        // everyday outcome, and must read as Ok(None), never as an error
        // (an up-to-date install lands in Idle, not Failed).
        let mut same = valid();
        same["version"] = json!(env!("CARGO_PKG_VERSION"));
        assert!(matches!(parse_and_validate(&serde_json::to_vec(&same).unwrap(),
            Channel::GithubLinux, None), Ok(None)));
        refused(valid(), |v| v["pub_date"] = json!("next Thursday"));
        refused(valid(), |v| v["notes_url"] = json!("https://evil.example/notes"));
        refused(valid(), |v| v["targets"] = json!({}));
        refused(valid(), |v| v["targets"].as_object_mut().unwrap().values_mut()
            .next().unwrap()["sha256"] = json!("no"));
        refused(valid(), |v| v["targets"].as_object_mut().unwrap().values_mut()
            .next().unwrap()["size"] = json!(super::super::MAX_ARTIFACT_SIZE + 1));
        refused(valid(), |v| v["targets"].as_object_mut().unwrap().values_mut()
            .next().unwrap()["size"] = json!(0));
        refused(valid(), |v| v["targets"].as_object_mut().unwrap().values_mut()
            .next().unwrap()["url"] = json!("https://evil.example/strop"));
        let highest = Version::parse("0.4.0").unwrap();
        assert!(parse_and_validate(&serde_json::to_vec(&valid()).unwrap(),
            Channel::GithubLinux, Some(&highest)).is_err());
    }

    #[test]
    fn any_trusted_key_and_signature_pair_is_enough() {
        let message = b"signed manifest bytes";
        let first = KeyPair::generate_unencrypted_keypair().unwrap();
        let second = KeyPair::generate_unencrypted_keypair().unwrap();
        let signature = sign(None, &second.sk, Cursor::new(message), None, None)
            .unwrap().into_string().into_bytes();
        let keys = [&first.pk, &second.pk].into_iter().map(|pk|
            PublicKey::from_base64(&pk.to_base64()).unwrap()).collect::<Vec<_>>();
        assert!(verify_signed(message, &[b"garbage".to_vec(), signature.clone()], &keys).is_ok());
        assert!(verify_signed(b"tampered", &[signature], &keys).is_err());
        assert!(verify_signed(message, &[], &keys).is_err());
        assert!(verify_signed(message, &[], &[]).is_err());
    }

    #[test]
    fn every_committed_key_line_bakes() {
        // The binary's trust anchor and the human-verifiable file are the
        // same bytes; a key line that fails to parse would silently shrink
        // the trusted set, so pin count(parsed) == count(key lines) >= 1.
        let key_lines = include_str!("../../../../minisign.pub").lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("untrusted comment"))
            .count();
        assert!(key_lines >= 1);
        assert_eq!(baked_keys().len(), key_lines);
    }

    #[test]
    fn rfc3339_edges_are_checked() {
        assert!(valid_rfc3339("2024-02-29T23:59:60Z"));
        assert!(valid_rfc3339("2026-07-18T12:34:56.123-03:30"));
        assert!(!valid_rfc3339("2026-02-29T12:00:00Z"));
        assert!(!valid_rfc3339("2026-07-18 12:00:00Z"));
        assert!(!valid_rfc3339("2026-07-18T24:00:00Z"));
    }
}
