# Interlingua Packet Contract

This is the human-facing schema receipt for Weksa interlingua packets.

The executable schema is [`crates/weksa/src/packet.rs`](../crates/weksa/src/packet.rs).
Its generated JSON Schemas are in [`schemas/cultnet/`](../schemas/cultnet/), and
a test fails when they drift. This document and the
[Interlingua Standard](../docs/interlingua-standard.md) describe it; where they
disagree with the types, the types win.

## Authority

The interlingua packet is the boundary between clients and language projects.

Clients submit structured meaning. Language projects consume that meaning,
project it through ontology, and render it through grammar, morphology,
phonology, diachrony, and renderer rules.

## Required Fields

- `id`: stable local identifier, the packet's store key.
- `schema_version` and `inventory_rev`: the schema and inventory revision the
  packet was written against.
- `kind`: one of the seven use cases in the standard.
- at least one semantic payload family:
  - `referents`
  - `predications`
  - `relations`

## Families

- `provenance`: source, corpus item, encoder and confidence.
- `context`: setting, medium, participants, audience.
- `discourse`: speech act, addressee and source-side nuance features.
- `constraints`: nodes to preserve, determinism, latency bound.
- `extensions`: namespaced client or setting pressure.
- `trace`: debug and authoring notes, stripped before lowering.

Every open slot is an inventory id, a node id, an entity id or a corpus item id.
The only free text is the trace, which the validator neither bounds nor reads
and which is stripped before lowering.

## Extension Contract

Extensions use explicit namespaces such as:

- `zyphos.*`
- `rtd.*`
- `game.*`
- `authoring.*`

Extensions are not a junk drawer. They are provisional fields awaiting evidence.
If a field becomes common across clients, promote it into the standard with a
version bump.

## Validation

`validate` in `crates/weksa/src/validate.rs` enforces rules V1 to V8 and V10 to V12
(V9 is retired), listed in the standard: schema version, payload present,
unique node ids, references that resolve, inventory ids that resolve at the
packet's revision, family matching the slot, feature value kind and slot, an
existing inventory revision, no repeated feature, extension, node reference or
role, and at most 64 nodes. A packet that breaks any rule is refused with a typed
refusal naming it, and never reaches a store.
