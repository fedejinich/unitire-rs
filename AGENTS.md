# AGENTS.md

## Scope
This file defines the collaboration and documentation contract for `/Users/void_rsk/Projects/unitrie-rs`.

## Mandatory documentation protocol (non-optional)
For every meaningful implementation change, always update all of these before finishing:
1. `/Users/void_rsk/Projects/unitrie-rs/TODO-CODEX.md`
2. `/Users/void_rsk/Projects/unitrie-rs/AGENTS.md` (if process rules evolve)
3. `/Users/void_rsk/Projects/unitrie-rs/docs/knowledge/zensical/evidence/project/worklog.md`
4. `/Users/void_rsk/Projects/unitrie-rs/docs/knowledge/zensical/evidence/project/worklog.json`

If the change modifies architecture, parity strategy, or performance gates, also update:
1. `/Users/void_rsk/Projects/unitrie-rs/docs/knowledge/zensical/maps/documentation-protocol.md`
2. the relevant detailed `.md` and `.json` evidence artifacts in `/Users/void_rsk/Projects/unitrie-rs/docs/knowledge/zensical/`.

## Required task planning format
Every execution plan must include:
1. dependency graph
2. task IDs
3. `depends_on: []`
4. explicit status (`todo`, `in_progress`, `done`)
5. Jira ticket per task

## Commit hygiene
1. Keep commits small and coherent.
2. Keep docs synchronized with code changes in the same PR/commit batch.
3. Never claim parity/performance milestones without evidence entries in Zensical KB.
