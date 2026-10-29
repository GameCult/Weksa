# Cultural Ontology Store

This directory holds draft Weksa cultural, subcultural, and source-referent
ontology profiles used for ontological lowering, syntax, grammar, discourse,
and surface-affordance selection.

Live pipeline shape:

1. Weksa stores reusable cultures, subcultures, and source referents here and
   in semantic retrieval storage such as Qdrant.
2. A Persona state references the relevant culture stack.
3. The Persona produces interlingua intent from current state and situation.
4. Weksa lowers that interlingua through the referenced culture stack.

Persona state owns current intent, memory pressure, and relationship stance.
Cultural ontology owns reusable salience, affordances, and constraints. Qdrant
may retrieve candidates, but accepted ontology remains typed state.

## Directory Roles

- `cultures/`: culture or life-history profiles that can shape lowering when a
  Persona explicitly references them.
- `referents/`: bounded source-text, mythic, literary, ritual, or historical
  referents. These are not whole-culture profiles; they preserve reusable
  role, salience, affordance, and forbidden-flattening pressure.
- `persona-stacks/`: named stacks that connect a Persona state surface to the
  profiles Weksa should load for lowering.

Shared source-tradition profiles should be parent profiles for narrower named
figure profiles. For example, Norse-derived Personas such as Mimir, Huginn,
Sindri, Odin, Vili, Loki, and Brokkr should be able to reference the shared
Eddic source-tradition profile while figure-specific child profiles preserve
the exact episode and role constraints that make each voice distinct.
