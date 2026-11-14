//! The validator owns the inventory and form rules a packet must meet. Every
//! path that admits a packet applies them, but `validate` only judges: it
//! returns `Ok(())` or the refusals. [`LoweringInput`], the only thing a
//! lowerer sees, comes from `PacketStore::admit` alone, which builds it from
//! the admitted packet and its corpus item through the crate-private
//! `LoweringInput::of`. The stance is checked like every feature
//! list, in slot `Stance`. The rules are V1..V8, V10..V12 and V14..V18: the
//! shape of an id or a grade is its type's parse, not a rule, so V9 is retired
//! and not renumbered; V13 is retired too. [`LoweringInput::with_scene`] reads the
//! scene of the view's own item group from the speaker-state store, and
//! `feature_refusals` is the one feature check it and that store share.
//!
//! `validate` owns form and capacity and makes no leak decision. A packet's one
//! free text is its trace (`notes` and `rejected_readings`): `validate` does not
//! bound or read it, and [`LoweringInput`] drops it, so no lowerer sees it.
//! Every other slot is an id type, so the lists are the bounded part. What bounds
//! each list (V11, V12): a feature list holds at
//! most one value per feature id and `extensions` at most one per extension id,
//! so each is at most the inventory's ids of that kind; `participants`,
//! `audience` and `preserve` hold each node id at most once and every id names
//! a node (V4), so each is at most `Packet::MAX_NODES`; a predication's `roles`
//! hold each role id at most once, so at most the inventory's roles; and the
//! nodes themselves are at most `Packet::MAX_NODES` (V12). Whether a score may
//! rise because corpus text reached a packet is owned by the golf loop's
//! context builder and the scorer (ruling `leak-owned-by-flow-and-scorer-3`),
//! never by a text rule here, in `admit_revision` or in `PacketStore::admit`.

use std::collections::BTreeSet;
use std::fmt;

use crate::corpus::CorpusItem;
use crate::inventory::{Family, InvId, Inventory, InventoryEntry, Slot, ValueKind};
use crate::packet::{
    Constraints, Context, Discourse, EntityId, Extension, FeatValue, FeatureValue, NodeId, Packet,
    PacketKind, Predication, ProvenanceSource, Referent, Relation, SCHEMA_VERSION, Stance,
};
use crate::store::{CorpusStore, SpeakerStateStore, StoreError};

/// A broken rule, named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// V1: the packet's schema_version is not the one these types define.
    V1SchemaVersion {
        found: u32,
    },
    /// V2: no referents, predications or relations.
    V2PayloadRequired,
    /// V3: a node id repeats across referents and predications.
    V3DuplicateNode {
        id: NodeId,
    },
    /// V4: a reference names no node.
    V4DanglingRef {
        id: NodeId,
        at: &'static str,
    },
    /// V5: an inventory id does not resolve at the packet's revision.
    V5Unresolved {
        id: InvId,
    },
    /// V6: an inventory id is of the wrong family for its slot.
    V6WrongFamily {
        id: InvId,
        expected: Family,
        at: &'static str,
    },
    /// V7: a feature or extension value does not fit its value kind.
    V7ValueKind {
        what: String,
    },
    /// V8: a feature is not declared for the slot it sits on.
    V8NotApplicable {
        feature: InvId,
        slot: Slot,
    },
    /// V10: the packet's inventory revision has no record.
    V10UnknownRevision {
        rev: u32,
    },
    /// V11: a feature id repeats in one slot's list (`slot` is `None` for an
    /// extension id repeated in the packet).
    V11DuplicateFeature {
        feature: InvId,
        slot: Option<Slot>,
    },
    /// V11: a node id repeats in `participants`, `audience` or `preserve`
    /// (`at` names the list).
    V11DuplicateNodeRef {
        id: NodeId,
        at: &'static str,
    },
    /// V11: a role id repeats in one predication's roles.
    V11DuplicateRole {
        role: InvId,
        predication: NodeId,
    },
    /// V12: more referents, predications and relations than `Packet::MAX_NODES`.
    V12TooManyNodes {
        count: usize,
        max: usize,
    },
    /// V14: an addressee consent that no addressee can have given, or that
    /// names a node that is not a predication. It is what the addressee asked
    /// for or agreed to, an act, so a referent cannot be its object.
    V14ConsentWithoutAddressee,
    V14ConsentNotAPredication {
        id: NodeId,
    },
    /// V15: an addressee consent set by a source that only read wording.
    /// A decomposition of text, or imported corpus text, sees "you asked her
    /// to" as words; only authoring or game telemetry saw the request.
    V15ConsentSource {
        source: ProvenanceSource,
    },
    /// V16: `with_scene` found its group's scene not covering the item's speaker
    /// or the speaker of some live item of the group.
    V16SpeakerNotInScene,
    /// V17: an addressee that names a predication. An act is not someone
    /// spoken to, so the addressee names a referent.
    V17AddresseeNotAReferent {
        id: NodeId,
    },
    /// V18: a predication whose frame is the inventory's `request_intervention`,
    /// with no `addressee_consent` naming it. It says what the addressee did, so
    /// only the packet's own record may license it.
    V18RequestWithoutConsent {
        predication: NodeId,
    },
}

impl Refusal {
    pub fn rule(&self) -> &'static str {
        match self {
            Refusal::V1SchemaVersion { .. } => "V1",
            Refusal::V2PayloadRequired => "V2",
            Refusal::V3DuplicateNode { .. } => "V3",
            Refusal::V4DanglingRef { .. } => "V4",
            Refusal::V5Unresolved { .. } => "V5",
            Refusal::V6WrongFamily { .. } => "V6",
            Refusal::V7ValueKind { .. } => "V7",
            Refusal::V8NotApplicable { .. } => "V8",
            Refusal::V10UnknownRevision { .. } => "V10",
            Refusal::V11DuplicateFeature { .. }
            | Refusal::V11DuplicateNodeRef { .. }
            | Refusal::V11DuplicateRole { .. } => "V11",
            Refusal::V12TooManyNodes { .. } => "V12",
            Refusal::V14ConsentWithoutAddressee | Refusal::V14ConsentNotAPredication { .. } => {
                "V14"
            }
            Refusal::V15ConsentSource { .. } => "V15",
            Refusal::V16SpeakerNotInScene => "V16",
            Refusal::V17AddresseeNotAReferent { .. } => "V17",
            Refusal::V18RequestWithoutConsent { .. } => "V18",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self {
            Refusal::V1SchemaVersion { .. } => "unsupported schema version",
            Refusal::V2PayloadRequired => "packet has no referents, predications or relations",
            Refusal::V3DuplicateNode { .. } => "a node id repeats",
            Refusal::V4DanglingRef { .. } => "a reference names no node",
            Refusal::V5Unresolved { .. } => {
                "an inventory id does not resolve at the packet's revision"
            }
            Refusal::V6WrongFamily { .. } => "an inventory id is of the wrong family for its slot",
            Refusal::V7ValueKind { .. } => "a value does not fit its value kind",
            Refusal::V8NotApplicable { .. } => "a feature is not declared for its slot",
            Refusal::V10UnknownRevision { .. } => "the inventory revision has no record",
            Refusal::V11DuplicateFeature { .. } => "a feature id repeats in one list",
            Refusal::V11DuplicateNodeRef { .. } => "a node id repeats in one list",
            Refusal::V11DuplicateRole { .. } => "a role id repeats in one predication",
            Refusal::V12TooManyNodes { .. } => "too many nodes",
            Refusal::V14ConsentWithoutAddressee => "consent given without an addressee",
            Refusal::V14ConsentNotAPredication { .. } => "consent names no predication",
            Refusal::V15ConsentSource { .. } => "consent set by a source that only read wording",
            Refusal::V16SpeakerNotInScene => {
                "the scene does not cover the speaker of every live item of its group"
            }
            Refusal::V17AddresseeNotAReferent { .. } => "the addressee names no referent",
            Refusal::V18RequestWithoutConsent { .. } => {
                "a request frame needs the addressee's consent naming it"
            }
        };
        write!(f, "{}: {what}", self.rule())
    }
}

/// Whether `packet` meets the rules under `inventory`. All broken rules are
/// returned. A pass carries no lowering view: that comes from
/// `PacketStore::admit` only.
///
/// ```
/// use weksa::{Inventory, Refusal, packet::Packet};
/// fn judge(p: &Packet, i: &Inventory) -> Result<(), Vec<Refusal>> { weksa::validate(p, i) }
/// ```
///
/// ```compile_fail
/// use weksa::{Inventory, LoweringInput, Refusal, packet::Packet};
/// fn lower(p: &Packet, i: &Inventory) -> Result<LoweringInput, Vec<Refusal>> { weksa::validate(p, i) }
/// ```
pub fn validate(packet: &Packet, inventory: &Inventory) -> Result<(), Vec<Refusal>> {
    let mut out = Vec::new();
    if packet.schema_version != SCHEMA_VERSION {
        out.push(Refusal::V1SchemaVersion {
            found: packet.schema_version,
        });
    }
    if packet.referents.is_empty() && packet.predications.is_empty() && packet.relations.is_empty()
    {
        out.push(Refusal::V2PayloadRequired);
    }
    let count = packet.referents.len() + packet.predications.len() + packet.relations.len();
    if count > Packet::MAX_NODES {
        out.push(Refusal::V12TooManyNodes {
            count,
            max: Packet::MAX_NODES,
        });
    }
    let nodes = check_nodes(packet, &mut out);
    check_refs(packet, &nodes, &mut out);
    check_addressee(packet, &nodes, &mut out);
    check_repeats(packet, &mut out);
    check_consent(packet, &nodes, &mut out);
    if inventory.has_revision(packet.inventory_rev) {
        Inventoried {
            inv: inventory,
            rev: packet.inventory_rev,
            out: &mut out,
        }
        .check(packet);
        check_requests(packet, inventory, &mut out);
    } else {
        out.push(Refusal::V10UnknownRevision {
            rev: packet.inventory_rev,
        });
    }
    if out.is_empty() { Ok(()) } else { Err(out) }
}

fn check_nodes(packet: &Packet, out: &mut Vec<Refusal>) -> BTreeSet<NodeId> {
    let mut nodes = BTreeSet::new();
    let ids = packet
        .referents
        .iter()
        .map(|r| &r.id)
        .chain(packet.predications.iter().map(|p| &p.id));
    for id in ids {
        if !nodes.insert(id.clone()) {
            out.push(Refusal::V3DuplicateNode { id: id.clone() });
        }
    }
    nodes
}

fn check_refs(packet: &Packet, nodes: &BTreeSet<NodeId>, out: &mut Vec<Refusal>) {
    let mut need = |id: &NodeId, at: &'static str| {
        if !nodes.contains(id) {
            out.push(Refusal::V4DanglingRef { id: id.clone(), at });
        }
    };
    for p in &packet.predications {
        for r in &p.roles {
            need(&r.filler, "role filler");
        }
    }
    for r in &packet.relations {
        need(&r.from, "relation from");
        need(&r.to, "relation to");
    }
    if let Some(a) = &packet.discourse.addressee {
        need(a, "addressee");
    }
    if let Some(c) = &packet.discourse.addressee_consent {
        need(c, "addressee consent");
    }
    for (list, at) in node_lists(packet) {
        for id in list {
            need(id, at);
        }
    }
}

/// V17: the addressee, when it names a node, names a referent. A dangling id
/// is V4's.
fn check_addressee(packet: &Packet, nodes: &BTreeSet<NodeId>, out: &mut Vec<Refusal>) {
    if let Some(a) = &packet.discourse.addressee
        && nodes.contains(a)
        && !packet.referents.iter().any(|r| &r.id == a)
    {
        out.push(Refusal::V17AddresseeNotAReferent { id: a.clone() });
    }
}

/// V14 and V15: the addressee's consent is a typed fact with one origin. A
/// dangling id is V4's, so V14 judges only a node that exists.
fn check_consent(packet: &Packet, nodes: &BTreeSet<NodeId>, out: &mut Vec<Refusal>) {
    let Some(consent) = &packet.discourse.addressee_consent else {
        return;
    };
    if packet.discourse.addressee.is_none() {
        out.push(Refusal::V14ConsentWithoutAddressee);
    }
    if nodes.contains(consent) && !packet.predications.iter().any(|p| &p.id == consent) {
        out.push(Refusal::V14ConsentNotAPredication {
            id: consent.clone(),
        });
    }
    match packet.provenance.source {
        ProvenanceSource::Authored | ProvenanceSource::GameTelemetry => {}
        source => out.push(Refusal::V15ConsentSource { source }),
    }
}

/// The gloss of the inventory frame that reports an addressee's request.
const REQUEST_FRAME_GLOSS: &str = "request_intervention";

/// V18: a predication whose frame, resolved at the packet's revision, is the
/// request frame is named by `addressee_consent`. An unresolved frame is V5's.
fn check_requests(packet: &Packet, inventory: &Inventory, out: &mut Vec<Refusal>) {
    for p in &packet.predications {
        if inventory
            .resolve(&p.frame, packet.inventory_rev)
            .is_some_and(|e| e.gloss == REQUEST_FRAME_GLOSS)
            && packet.discourse.addressee_consent.as_ref() != Some(&p.id)
        {
            out.push(Refusal::V18RequestWithoutConsent {
                predication: p.id.clone(),
            });
        }
    }
}

fn node_lists(packet: &Packet) -> [(&Vec<NodeId>, &'static str); 3] {
    [
        (&packet.context.participants, "participant"),
        (&packet.context.audience, "audience"),
        (&packet.constraints.preserve, "preserve"),
    ]
}

/// V11 for the lists that hold nodes and roles: each node id once per node list,
/// each role id once per predication.
fn check_repeats(packet: &Packet, out: &mut Vec<Refusal>) {
    for (list, at) in node_lists(packet) {
        let mut seen = BTreeSet::new();
        for id in list {
            if !seen.insert(id) {
                out.push(Refusal::V11DuplicateNodeRef { id: id.clone(), at });
            }
        }
    }
    for p in &packet.predications {
        let mut seen = BTreeSet::new();
        for r in &p.roles {
            if !seen.insert(r.role) {
                out.push(Refusal::V11DuplicateRole {
                    role: r.role,
                    predication: p.id.clone(),
                });
            }
        }
    }
}

/// The inventory-dependent rules V5..V8 and V11, read at the packet's revision.
struct Inventoried<'a> {
    inv: &'a Inventory,
    rev: u32,
    out: &'a mut Vec<Refusal>,
}

impl<'a> Inventoried<'a> {
    fn check(&mut self, p: &Packet) {
        let Packet {
            context,
            discourse,
            referents,
            predications,
            relations,
            extensions,
            stance,
            ..
        } = p;
        let Context {
            setting, medium, ..
        } = context;
        for id in setting.iter().chain(medium) {
            self.slot(id, Family::Concept, "context concept");
        }
        let Discourse {
            speech_act,
            features,
            ..
        } = discourse;
        if let Some(id) = speech_act {
            self.slot(id, Family::SpeechAct, "speech act");
        }
        self.features(features, Slot::Discourse);
        self.features(&stance.features, Slot::Stance);
        for Referent {
            kind,
            concept,
            features,
            ..
        } in referents
        {
            self.slot(kind, Family::ReferentKind, "referent kind");
            self.slot(concept, Family::Concept, "referent concept");
            self.features(features, Slot::Referent);
        }
        for Predication {
            concept,
            frame,
            roles,
            features,
            ..
        } in predications
        {
            self.slot(concept, Family::Concept, "predication concept");
            self.slot(frame, Family::Frame, "frame");
            for r in roles {
                self.slot(&r.role, Family::Role, "role");
            }
            self.features(features, Slot::Predication);
        }
        for Relation {
            relation, features, ..
        } in relations
        {
            self.slot(relation, Family::Relation, "relation");
            self.features(features, Slot::Relation);
        }
        let mut seen = BTreeSet::new();
        for Extension { key, value } in extensions {
            if !seen.insert(*key) {
                self.out.push(Refusal::V11DuplicateFeature {
                    feature: *key,
                    slot: None,
                });
            }
            if let Some(entry) = self.slot(key, Family::Extension, "extension")
                && !fits(entry.value_kind.as_ref(), value)
            {
                self.out.push(Refusal::V7ValueKind {
                    what: key.to_string(),
                });
            }
        }
    }

    /// V5, then V6 when it resolved.
    fn slot(&mut self, id: &InvId, want: Family, at: &'static str) -> Option<&'a InventoryEntry> {
        let entry = self.resolve(id)?;
        if entry.family() != want {
            self.out.push(Refusal::V6WrongFamily {
                id: *id,
                expected: want,
                at,
            });
            return None;
        }
        Some(entry)
    }

    fn resolve(&mut self, id: &InvId) -> Option<&'a InventoryEntry> {
        let found = self.inv.resolve(id, self.rev);
        if found.is_none() {
            self.out.push(Refusal::V5Unresolved { id: *id });
        }
        found
    }

    fn features(&mut self, features: &[FeatureValue], slot: Slot) {
        let mut seen = BTreeSet::new();
        for f in features {
            if !seen.insert(f.feature) {
                self.out.push(Refusal::V11DuplicateFeature {
                    feature: f.feature,
                    slot: Some(slot),
                });
            }
            let Some(entry) = self.slot(&f.feature, Family::Feature, "feature") else {
                continue;
            };
            if !entry.applies_to.contains(&slot) {
                self.out.push(Refusal::V8NotApplicable {
                    feature: f.feature,
                    slot,
                });
            }
            if !fits(entry.value_kind.as_ref(), &f.value) {
                self.out.push(Refusal::V7ValueKind {
                    what: f.feature.to_string(),
                });
            }
        }
    }
}

/// The one feature check of the speaker-state documents and `with_scene`: V10
/// when `rev` has no record, else the rules the packet's own check applies to a
/// feature list in `slot` (V5 to V8, V11).
pub(crate) fn feature_refusals(
    inventory: &Inventory,
    rev: u32,
    features: &[FeatureValue],
    slot: Slot,
) -> Vec<Refusal> {
    let mut out = Vec::new();
    if !inventory.has_revision(rev) {
        out.push(Refusal::V10UnknownRevision { rev });
    } else {
        Inventoried {
            inv: inventory,
            rev,
            out: &mut out,
        }
        .features(features, slot);
    }
    out
}

fn fits(kind: Option<&ValueKind>, value: &FeatValue) -> bool {
    match (kind, value) {
        (Some(ValueKind::Grade), FeatValue::Grade(_)) => true,
        (Some(ValueKind::Options(labels)), FeatValue::Option(l)) => labels.contains(l),
        (Some(ValueKind::Flag), FeatValue::Flag(_)) => true,
        _ => false,
    }
}

/// What `with_scene` found for a view: the scene revision `rev` it read, or why
/// it selected no relation. Each case is distinct so a missing annotation or a
/// store at the wrong path cannot look like a scene that held nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneRead {
    /// `with_scene` has not run.
    Unread,
    /// The store holds no scene for the item's group.
    NoScene,
    /// The scene was read and the packet names no addressee.
    NoAddressee { rev: u32 },
    /// The addressee referent is the speaker themself.
    SelfAddressed { rev: u32 },
    /// The scene holds no relation from the speaker to this addressee.
    NoRelation { rev: u32 },
    /// The relation was found; its features may be none.
    Selected {
        rev: u32,
        features: Vec<FeatureValue>,
    },
}

/// What the lowerer sees: a validated packet's meaning, with no packet id, no
/// provenance (so no corpus item key, encoder or source) and no trace. Its
/// fields are private and the constructor is crate-private, called only by
/// `PacketStore::admit`, so a lowerer cannot be handed an unadmitted packet or
/// the key to an answer text.
///
/// Each field has a doctest that reads it from outside the crate. A read
/// compiles exactly when the field is `pub`, so the doctest (which must fail to
/// compile) fails as soon as any one field is made public. A struct literal
/// cannot pin this: it fails to compile while any other field stays private,
/// and stable rustdoc does not check the error code.
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.inventory_rev; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.kind; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.context; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.discourse; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.referents; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.predications; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.relations; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.constraints; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.extensions; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.stance; }
/// ```
///
/// ```compile_fail
/// fn read(l: weksa::LoweringInput) { let _ = l.scene; }
/// ```
///
/// Scene values come only from the store: the call names neither speaker nor
/// scene. The first block is the call with the store; the second hands it a
/// scene value and a speaker and fails for that alone.
///
/// ```
/// fn pick(
///     l: weksa::LoweringInput,
///     i: &weksa::Inventory,
///     s: &weksa::SpeakerStateStore,
///     c: &weksa::CorpusStore,
/// ) {
///     let _ = l.with_scene(i, s, c);
/// }
/// ```
///
/// ```compile_fail
/// fn pick(
///     l: weksa::LoweringInput,
///     i: &weksa::Inventory,
///     s: &weksa::speaker::Scene,
///     e: &weksa::EntityId,
/// ) {
///     let _ = l.with_scene(i, s, e);
/// }
/// ```
///
/// ```compile_fail
/// fn pick(l: weksa::LoweringInput, i: &weksa::Inventory) {
///     let _ = l.with_scene(i, vec![]);
/// }
/// ```
///
/// The constructor is private: a call from outside the crate compiles only if
/// `LoweringInput::of` is `pub`. The first block is the same code without the
/// call, so the second fails for that call and nothing else.
///
/// ```
/// fn build(p: &weksa::packet::Packet, item: &weksa::CorpusItem) { let _ = (p, item); }
/// ```
///
/// ```compile_fail
/// fn build(p: &weksa::packet::Packet, item: &weksa::CorpusItem) {
///     let _ = weksa::LoweringInput::of(p, item);
/// }
/// ```
///
/// A lowering view is never read from bytes: `Deserialize` would let a caller
/// build one without `validate`. The first block proves the bound resolves for
/// a type that does implement it.
///
/// ```
/// fn de<T: serde::de::DeserializeOwned>() {}
/// de::<weksa::packet::Packet>();
/// ```
///
/// ```compile_fail
/// fn de<T: serde::de::DeserializeOwned>() {}
/// de::<weksa::LoweringInput>();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweringInput {
    inventory_rev: u32,
    kind: PacketKind,
    context: Context,
    discourse: Discourse,
    referents: Vec<Referent>,
    predications: Vec<Predication>,
    relations: Vec<Relation>,
    constraints: Constraints,
    extensions: Vec<Extension>,
    stance: Stance,
    /// The item's speaker and group, as the corpus store holds them: the
    /// only source of whose relation `with_scene` selects, and from which scene.
    speaker: EntityId,
    group: String,
    scene: SceneRead,
}

impl LoweringInput {
    /// The view of an admitted packet and the corpus item it was minted for.
    pub(crate) fn of(p: &Packet, item: &CorpusItem) -> Self {
        Self {
            inventory_rev: p.inventory_rev,
            kind: p.kind,
            context: p.context.clone(),
            discourse: p.discourse.clone(),
            referents: p.referents.clone(),
            predications: p.predications.clone(),
            relations: p.relations.clone(),
            constraints: p.constraints.clone(),
            extensions: p.extensions.clone(),
            stance: p.stance.clone(),
            speaker: item.speaker.clone(),
            group: item.group.clone(),
            scene: SceneRead::Unread,
        }
    }

    /// Reads the speaker's relation to the addressee, the only path for scene
    /// values into a view. The speaker is the item's speaker and the scene is
    /// the latest the store holds for the item's group; the caller names
    /// neither. A scene that does not list the item's speaker, or the speaker of
    /// any live item of the group (the rule `put_scene` applies), is V16, with
    /// or without an addressee. What the read found is recorded on the view as a
    /// [`SceneRead`], never an empty feature list standing for five cases:
    /// no scene for the group, no addressee, an addressee who is the speaker,
    /// no such relation, or the relation from the speaker to the addressee
    /// referent's entity (to `Unnamed` when the referent has none), whose
    /// features are checked like every feature list in slot `Scene` at this
    /// view's inventory revision. A read replaces any earlier one. The English
    /// lowerer ignores it.
    pub fn with_scene(
        mut self,
        inventory: &Inventory,
        state: &SpeakerStateStore,
        corpus: &CorpusStore,
    ) -> Result<Self, StoreError> {
        self.scene = SceneRead::Unread;
        let Some(scene) = state.scene(&self.group)? else {
            self.scene = SceneRead::NoScene;
            return Ok(self);
        };
        let rev = scene.n;
        let live = corpus.all()?;
        let live = live
            .iter()
            .filter(|i| i.group == self.group && !i.withdrawn)
            .map(|i| &i.speaker);
        if scene
            .uncovered(live.chain(std::iter::once(&self.speaker)))
            .is_some()
        {
            return Err(StoreError::Invalid(vec![Refusal::V16SpeakerNotInScene]));
        }
        let Some(addressee) = self.discourse.addressee.as_ref() else {
            self.scene = SceneRead::NoAddressee { rev };
            return Ok(self);
        };
        let entity = self
            .referents
            .iter()
            .find(|r| &r.id == addressee)
            .and_then(|r| r.entity.as_ref());
        if entity == Some(&self.speaker) {
            self.scene = SceneRead::SelfAddressed { rev };
            return Ok(self);
        }
        let Some(features) = scene.relation(&self.speaker, entity) else {
            self.scene = SceneRead::NoRelation { rev };
            return Ok(self);
        };
        let out = feature_refusals(inventory, self.inventory_rev, features, Slot::Scene);
        if !out.is_empty() {
            return Err(StoreError::Invalid(out));
        }
        self.scene = SceneRead::Selected {
            rev,
            features: features.to_vec(),
        };
        Ok(self)
    }

    pub fn inventory_rev(&self) -> u32 {
        self.inventory_rev
    }
    pub fn kind(&self) -> PacketKind {
        self.kind
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn discourse(&self) -> &Discourse {
        &self.discourse
    }
    pub fn referents(&self) -> &[Referent] {
        &self.referents
    }
    pub fn predications(&self) -> &[Predication] {
        &self.predications
    }
    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }
    pub fn constraints(&self) -> &Constraints {
        &self.constraints
    }
    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }
    pub fn stance(&self) -> &Stance {
        &self.stance
    }
    /// The features of the scene relation from the speaker to the addressee.
    /// Empty for every [`SceneRead`] but `Selected` (and for a selected
    /// relation that has no features): ask [`scene_read`](Self::scene_read) which.
    pub fn scene(&self) -> &[FeatureValue] {
        match &self.scene {
            SceneRead::Selected { features, .. } => features,
            _ => &[],
        }
    }
    /// What `with_scene` read; `Unread` until it ran.
    pub fn scene_read(&self) -> &SceneRead {
        &self.scene
    }
}
