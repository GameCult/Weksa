//! The validator owns the inventory and form rules a packet must meet. Every
//! path that admits a packet applies them, but `validate` only judges: it
//! returns `Ok(())` or the refusals. [`LoweringInput`], the only thing a
//! lowerer sees, comes from `PacketStore::admit` alone, which builds it through
//! the crate-private `admissible`. The stance is checked like every feature
//! list, in slot `Stance`. The rules are V1..V8 and V10..V12: the shape of an id
//! or a grade is its type's parse, not a rule, so V9 is retired and not
//! renumbered.
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

use crate::inventory::{Family, InvId, Inventory, InventoryEntry, Slot, ValueKind};
use crate::packet::{
    Constraints, Context, Discourse, Extension, FeatValue, FeatureValue, NodeId, Packet,
    PacketKind, Predication, Referent, Relation, SCHEMA_VERSION, Stance,
};

/// A broken rule, named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// V1: the packet's schema_version is not the one these types define.
    V1SchemaVersion { found: u32 },
    /// V2: no referents, predications or relations.
    V2PayloadRequired,
    /// V3: a node id repeats across referents and predications.
    V3DuplicateNode { id: NodeId },
    /// V4: a reference names no node.
    V4DanglingRef { id: NodeId, at: &'static str },
    /// V5: an inventory id does not resolve at the packet's revision.
    V5Unresolved { id: InvId },
    /// V6: an inventory id is of the wrong family for its slot.
    V6WrongFamily {
        id: InvId,
        expected: Family,
        at: &'static str,
    },
    /// V7: a feature or extension value does not fit its value kind.
    V7ValueKind { what: String },
    /// V8: a feature is not declared for the slot it sits on.
    V8NotApplicable { feature: InvId, slot: Slot },
    /// V10: the packet's inventory revision has no record.
    V10UnknownRevision { rev: u32 },
    /// V11: a feature id repeats in one slot's list (`slot` is `None` for an
    /// extension id repeated in the packet).
    V11DuplicateFeature { feature: InvId, slot: Option<Slot> },
    /// V11: a node id repeats in `participants`, `audience` or `preserve`
    /// (`at` names the list).
    V11DuplicateNodeRef { id: NodeId, at: &'static str },
    /// V11: a role id repeats in one predication's roles.
    V11DuplicateRole { role: InvId, predication: NodeId },
    /// V12: more referents, predications and relations than `Packet::MAX_NODES`.
    V12TooManyNodes { count: usize, max: usize },
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
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {self:?}", self.rule())
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
    admissible(packet, inventory).map(|_| ())
}

/// The one implementation of the rules. Crate-private: `PacketStore::admit` is
/// the only caller that keeps the view.
///
/// ```compile_fail
/// use weksa::{Inventory, packet::Packet};
/// fn lower(p: &Packet, i: &Inventory) { let _ = weksa::validate::admissible(p, i); }
/// ```
pub(crate) fn admissible(
    packet: &Packet,
    inventory: &Inventory,
) -> Result<LoweringInput, Vec<Refusal>> {
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
    check_repeats(packet, &mut out);
    if inventory.has_revision(packet.inventory_rev) {
        Inventoried {
            inv: inventory,
            rev: packet.inventory_rev,
            out: &mut out,
        }
        .check(packet);
    } else {
        out.push(Refusal::V10UnknownRevision {
            rev: packet.inventory_rev,
        });
    }
    if out.is_empty() {
        Ok(LoweringInput::of(packet))
    } else {
        Err(out)
    }
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
    for (list, at) in node_lists(packet) {
        for id in list {
            need(id, at);
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

fn fits(kind: Option<&ValueKind>, value: &FeatValue) -> bool {
    match (kind, value) {
        (Some(ValueKind::Grade), FeatValue::Grade(_)) => true,
        (Some(ValueKind::Options(labels)), FeatValue::Option(l)) => labels.contains(l),
        (Some(ValueKind::Flag), FeatValue::Flag(_)) => true,
        _ => false,
    }
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
/// The constructor is private: a call from outside the crate compiles only if
/// `LoweringInput::of` is `pub`. The first block is the same code without the
/// call, so the second fails for that call and nothing else.
///
/// ```
/// fn build(p: &weksa::packet::Packet) { let _ = p; }
/// ```
///
/// ```compile_fail
/// fn build(p: &weksa::packet::Packet) { let _ = weksa::LoweringInput::of(p); }
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
}

impl LoweringInput {
    fn of(p: &Packet) -> Self {
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
        }
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
}
