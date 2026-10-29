//! Weksa's typed interlingua state: packet, inventory and corpus documents in
//! CultCache, the validator that decides whether a packet exists, the derived
//! train/validation/test split, the stores that admit them, and the scorer
//! (chrF, meaning cosine, leak diagnostics).

pub mod corpus;
pub mod entity;
pub mod form;
pub mod inventory;
pub mod packet;
pub mod schema;
pub mod score;
pub mod split;
pub mod store;
pub mod validate;

pub use corpus::{CorpusItem, CorpusItemId, NameSpan, Source, Turn};
pub use entity::{Entity, EntityOrigin, LanguageForms};
pub use form::FormError;
pub use inventory::{
    Change, Family, InvId, Inventory, InventoryEntry, InventoryRevision, OptionLabel, OptionSet,
    Slot, ValueKind,
};
pub use packet::{
    EncoderId, EntityId, Grade, NodeId, Packet, PacketId, PacketKind, SCHEMA_VERSION,
};
pub use score::{Embedder, ItemScore, OllamaEmbedder, Scorer, chrf};
pub use split::{Split, SplitScheme};
pub use store::{
    AddOutcome, CorpusStore, EntityBound, EntityStore, InventoryStore, PacketStore, StoreError,
    StoreRefusal,
};
pub use validate::{LoweringInput, Refusal, validate};
