//! JSON Schema publication. JSON is only the xenos-boundary export of the typed
//! documents; nothing in this crate reads JSON.

use crate::corpus::CorpusItem;
use crate::entity::Entity;
use crate::inventory::{InventoryEntry, InventoryRevision};
use crate::packet::Packet;
use cultcache_rs::DatabaseEntry;
use schemars::{JsonSchema, schema_for};

fn published_for<T: DatabaseEntry + JsonSchema>() -> (String, String) {
    let json = serde_json::to_string_pretty(&schema_for!(T)).expect("a schema serializes");
    (format!("{}.schema.json", T::TYPE), format!("{json}\n"))
}

/// `(file name, contents)` for every document type, as `schemas/cultnet/` holds them.
pub fn published() -> Vec<(String, String)> {
    vec![
        published_for::<Packet>(),
        published_for::<InventoryEntry>(),
        published_for::<InventoryRevision>(),
        published_for::<CorpusItem>(),
        published_for::<Entity>(),
    ]
}
