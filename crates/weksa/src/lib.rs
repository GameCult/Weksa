//! Weksa's typed interlingua state: packet, inventory and corpus documents in
//! CultCache, the validator that decides whether a packet exists, the derived
//! train/validation/test split, the stores that admit them, and the scorer
//! (chrF, meaning cosine, leak diagnostics).

pub mod context;
pub mod corpus;
pub mod daodejing;
pub mod encode;
pub mod entity;
pub mod form;
pub mod inventory;
pub mod lower;
pub mod model;
pub mod ontology;
pub mod packet;
pub mod phones;
pub mod run;
pub mod schema;
pub mod score;
pub mod speaker;
pub mod split;
pub mod store;
pub mod validate;

pub use corpus::{CorpusItem, CorpusItemId, NameSpan, Source, Turn};
pub use encode::{EditDraft, LoweringDraft, PacketDraft};
pub use entity::{Entity, EntityOrigin, LanguageForms, Pronunciation};
pub use form::FormError;
pub use inventory::{
    Change, Family, InvId, Inventory, InventoryEntry, InventoryRevision, OptionLabel, OptionSet,
    Slot, ValueKind,
};
pub use lower::{EntityForms, LowerError, PhoneLine, TARGET_LANGUAGE, lower, phones, pronounce};
pub use model::{AnswerFiles, ModelPort};
pub use ontology::{Condition, Layer, Ontology, OntologyEntry, Test, select};
pub use packet::{
    EncoderId, EntityId, Grade, NodeId, Packet, PacketId, PacketKind, SCHEMA_VERSION, Stance,
};
pub use phones::{Phone, PhoneInventory, SegmentError, Stress};
pub use run::{Decision, RoundRecord, RunStores, cycle, round};
pub use score::{Embedder, ItemScore, OllamaEmbedder, Scorer, chrf};
pub use speaker::{Annotator, PairRelation, Party, Scene, Speaker};
pub use split::{Split, SplitScheme};
pub use store::{
    AddOutcome, CorpusStore, EntityBound, EntityStore, InventoryStore, OntologyStore, PacketStore,
    SpeakerStateStore, StoreError, StoreRefusal,
};
pub use validate::{LoweringInput, Refusal, SceneRead, validate};
