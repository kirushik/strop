use serde::Serialize;
use serde_json::Value;
use strop_core::document::BlockKind;
use strop_core::store::{Loaded, Store};

#[derive(Debug, PartialEq, Serialize)]
pub struct SemanticProjection {
    text: String,
    spans: Value,
    block_kinds: Value,
    note_anchors: Value,
    checkpoints: Vec<CheckpointProjection>,
    materialized_state: Option<Value>,
    journal: Value,
    graveyard: Value,
    asset_refs: Vec<AssetProjection>,
    provenance: Value,
}

#[derive(Debug, PartialEq, Serialize)]
struct CheckpointProjection {
    name: String,
    timestamp_ms: i64,
    manual: bool,
}

#[derive(Debug, PartialEq, Serialize)]
struct AssetProjection {
    id: String,
    present: bool,
    bytes_blake3: Option<String>,
}

pub fn project(store: &Store, loaded: Loaded) -> SemanticProjection {
    let checkpoints = store.checkpoints();
    let materialized_state = checkpoints.iter().find_map(|checkpoint| {
        store.checkpoint_state(checkpoint).map(|(text, spans, blocks)| {
            serde_json::json!({
                "text": text,
                "spans": spans,
                "block_kinds": blocks.kinds(),
            })
        })
    });
    let mut refs = Vec::new();
    for kind in loaded.blocks.kinds() {
        if let BlockKind::Image { src, .. } = kind {
            refs.push(src.clone());
        }
    }
    for entry in loaded.graveyard.entries() {
        refs.extend(entry.kinds.iter().filter_map(|kind| match kind {
            BlockKind::Image { src, .. } => Some(src.clone()),
            _ => None,
        }));
    }
    refs.sort();
    refs.dedup();
    let asset_refs = refs
        .into_iter()
        .map(|id| {
            let bytes = store.get_asset(&id);
            AssetProjection {
                present: bytes.is_some(),
                bytes_blake3: bytes.as_ref().map(|b| blake3::hash(b).to_hex().to_string()),
                id,
            }
        })
        .collect();
    SemanticProjection {
        text: loaded.text,
        spans: serde_json::to_value(loaded.spans).unwrap(),
        block_kinds: serde_json::to_value(loaded.blocks.kinds()).unwrap(),
        note_anchors: serde_json::to_value(loaded.annotations).unwrap(),
        checkpoints: checkpoints
            .iter()
            .map(|checkpoint| CheckpointProjection {
                name: checkpoint.name.clone(),
                timestamp_ms: checkpoint.timestamp_ms(),
                manual: checkpoint.manual,
            })
            .collect(),
        materialized_state,
        journal: serde_json::to_value(loaded.journal).unwrap(),
        graveyard: serde_json::to_value(loaded.graveyard).unwrap(),
        asset_refs,
        provenance: serde_json::to_value(loaded.provenance).unwrap(),
    }
}
