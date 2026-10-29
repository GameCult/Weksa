//! Weksa's typed interlingua state: packet, inventory and corpus documents in
//! CultCache, the validator that decides whether a packet exists, the derived
//! train/validation/test split, and the stores that admit them.

pub mod corpus;
pub mod form;
pub mod inventory;
pub mod packet;
pub mod schema;
pub mod split;
pub mod store;
pub mod validate;

pub use corpus::{CorpusItem, Source, Turn};
pub use form::FormError;
pub use inventory::{
    Change, Family, InvId, Inventory, InventoryEntry, InventoryRevision, OptionLabel, OptionSet,
    Slot, ValueKind,
};
pub use packet::{
    EncoderId, Grade, NodeId, Packet, PacketId, PacketKind, ProperName, SCHEMA_VERSION,
};
pub use split::{Split, SplitScheme};
pub use store::{AddOutcome, CorpusStore, InventoryStore, PacketStore, StoreError, StoreRefusal};
pub use validate::{LoweringInput, Refusal, validate};
