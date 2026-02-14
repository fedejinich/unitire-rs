# Unitrie-rs Publish TODO

Status date: 2026-02-14

## Dependency graph

```mermaid
graph TD
  T1["T1 (RSKJ-2498) Prepare package metadata and docs"]
  T2["T2 (RSKJ-2499) Align crate naming to standalone unitrie-rs"]
  T3["T3 (RSKJ-2500) Validate standalone build/tests"]
  T4["T4 (RSKJ-2501) Configure Git remote and push"]
  T5["T5 (RSKJ-2502) Final verification and status report"]

  T1 --> T2
  T2 --> T3
  T3 --> T4
  T4 --> T5
```

## Execution TODO list

- [x] `T1` `status: done` `depends_on: []` `jira: RSKJ-2498`
  - Add production-ready README/licensing metadata for the standalone core crate.
- [x] `T2` `status: done` `depends_on: [T1]` `jira: RSKJ-2499`
  - Rename crate package/import path to `unitrie-rs` and update internal references.
- [x] `T3` `status: done` `depends_on: [T2]` `jira: RSKJ-2500`
  - Run `cargo test` and ensure bench target still resolves.
- [ ] `T4` `status: in_progress` `depends_on: [T3]` `jira: RSKJ-2501`
  - Set `origin` to `git@github.com:fedejinich/unitire-rs.git` and push `main`.
- [ ] `T5` `status: todo` `depends_on: [T4]` `jira: RSKJ-2502`
  - Confirm remote state and finalize publication status.
