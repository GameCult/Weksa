//! The English lowerer: the validated lowering view of a packet to text, by a
//! fixed table keyed by inventory ids. It renders predications, referents and the
//! speech act, and nothing else: no style, no model, no corpus text. It takes a
//! [`LoweringInput`], so it cannot see the trace or the corpus item. A packet
//! carrying a feature, relation, extension, role fill or referent it does not
//! render is refused, because rendering part of a meaning would assert a different
//! one (`polarity: negative` dropped says the opposite). Context, kind and
//! constraints steer delivery and are not rendered; the concept of a predication
//! must resolve but the frame owns the verb.
//!
//! The table is a temporary owner. Each row speaks for an inventory entry and
//! names the gloss it speaks for, so a row cannot outlive a change of what its id
//! means. `language-ontology` moves the rows into ontology documents.

use std::fmt;

use crate::inventory::{Family, InvId, Inventory};
use crate::packet::{NodeId, Predication};
use crate::validate::LoweringInput;

/// `(inventory id, the gloss the row speaks for, surface)`.
type Row = (&'static str, &'static str, &'static str);

/// Referent concepts, as the noun phrase after "the".
const CONCEPTS: &[Row] = &[
    ("concept:3", "player", "player"),
    ("concept:4", "control", "control"),
    ("concept:5", "ship_mind", "ship mind"),
];

/// Frames: the clause a predication of that frame says. `{gloss}` is the filler
/// of the role whose inventory gloss it is.
const FRAMES: &[Row] = &[
    (
        "frame:1",
        "hazard_warning",
        "{agent} may endanger {patient} by touching {instrument}",
    ),
    (
        "frame:2",
        "intervention",
        "{agent} will prevent {patient} to protect {beneficiary}",
    ),
];

/// Speech acts: the opener of the first sentence.
const SPEECH_ACTS: &[Row] = &[
    ("speech_act:1", "assert", ""),
    ("speech_act:5", "warn", "Warning: "),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    /// The packet carries a slot the lowerer does not render.
    Unrendered { what: &'static str },
    /// No table row for an id the packet uses.
    NoRow { id: InvId },
    /// The row's gloss is not the gloss the inventory gives its id at the
    /// packet's revision, or the id does not resolve there.
    GlossMismatch { id: InvId, row: &'static str },
    /// A frame template names a role the inventory has no live entry for.
    UnknownRole { frame: InvId, role: String },
    /// The predication fills no role its frame's template needs.
    MissingRole { predication: NodeId, role: String },
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LowerError {}

fn row(
    table: &'static [Row],
    id: InvId,
    inv: &Inventory,
    rev: u32,
) -> Result<&'static str, LowerError> {
    let (_, gloss, surface) = table
        .iter()
        .find(|(key, _, _)| *key == id.to_string())
        .ok_or(LowerError::NoRow { id })?;
    match inv.resolve(&id, rev) {
        Some(e) if e.gloss == *gloss => Ok(surface),
        _ => Err(LowerError::GlossMismatch { id, row: gloss }),
    }
}

pub fn lower(input: &LoweringInput, inv: &Inventory) -> Result<String, LowerError> {
    let rev = input.inventory_rev();
    let unrendered = |what| Err(LowerError::Unrendered { what });
    if input.predications().is_empty() {
        return unrendered("no predication");
    }
    if !input.discourse().features.is_empty()
        || input.referents().iter().any(|r| !r.features.is_empty())
        || input.predications().iter().any(|p| !p.features.is_empty())
    {
        return unrendered("features");
    }
    if !input.relations().is_empty() {
        return unrendered("relations");
    }
    if !input.extensions().is_empty() {
        return unrendered("extensions");
    }
    let opener = match input.discourse().speech_act {
        Some(act) => row(SPEECH_ACTS, act, inv, rev)?,
        None => "",
    };
    let mut sentences = Vec::new();
    for p in input.predications() {
        sentences.push(clause(input, inv, p)?);
    }
    let mentioned = |r: &NodeId| {
        input
            .predications()
            .iter()
            .any(|p| p.roles.iter().any(|f| &f.filler == r))
    };
    if input.referents().iter().any(|r| !mentioned(&r.id)) {
        return unrendered("a referent no role mentions");
    }
    let mut out = String::new();
    for (i, clause) in sentences.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if i == 0 {
            out.push_str(opener);
        }
        let mut chars = clause.chars();
        if let Some(first) = chars.next() {
            if i == 0 && !opener.is_empty() {
                out.push(first);
            } else {
                out.extend(first.to_uppercase());
            }
            out.push_str(chars.as_str());
        }
        out.push('.');
    }
    Ok(out)
}

fn clause(input: &LoweringInput, inv: &Inventory, p: &Predication) -> Result<String, LowerError> {
    let rev = input.inventory_rev();
    let template = row(FRAMES, p.frame, inv, rev)?;
    let mut said: Vec<&NodeId> = Vec::new();
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}').map_or(rest.len(), |c| open + c);
        let name = &rest[open + 1..close];
        rest = &rest[(close + 1).min(rest.len())..];
        let role = inv
            .entries()
            .find(|e| e.family() == Family::Role && e.gloss == name && e.live_at(rev))
            .ok_or_else(|| LowerError::UnknownRole {
                frame: p.frame,
                role: name.to_string(),
            })?;
        let fill =
            p.roles
                .iter()
                .find(|r| r.role == role.id)
                .ok_or_else(|| LowerError::MissingRole {
                    predication: p.id.clone(),
                    role: name.to_string(),
                })?;
        out.push_str(&filler(input, inv, &fill.filler, &said)?);
        said.push(&fill.filler);
    }
    out.push_str(rest);
    if p.roles.len() != said.len() {
        return Err(LowerError::Unrendered {
            what: "a role the frame does not name",
        });
    }
    Ok(out)
}

/// A role filler as a phrase: the addressee is "you", a predication is "that",
/// another referent is "the" and its concept word. The addressee said twice in
/// one predication is reflexive; no other referent is, because the packet has no
/// animacy to choose "themself" or "itself" by.
fn filler(
    input: &LoweringInput,
    inv: &Inventory,
    node: &NodeId,
    said: &[&NodeId],
) -> Result<String, LowerError> {
    let again = said.contains(&node);
    if let Some(r) = input.referents().iter().find(|r| &r.id == node) {
        if input.discourse().addressee.as_ref() == Some(node) {
            return Ok(if again { "yourself" } else { "you" }.to_string());
        }
        let word = row(CONCEPTS, r.concept, inv, input.inventory_rev())?;
        return Ok(format!("the {word}"));
    }
    Ok("that".to_string())
}
