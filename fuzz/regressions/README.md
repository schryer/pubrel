# Fuzzing regressions

Inputs that once made a target fail. `make fuzz-smoke` replays them before
fuzzing anything new, so a bug they found cannot quietly return.

| Target | Input | What it found |
|---|---|---|
| `manifest` | `version-at-u64-max` | `version::next` bumped a part of the previous version already at `u64::MAX`: a panic wherever overflow is checked, as in pubrel's release builds. A pull request could make `check` crash instead of refusing the version. |

Each also has a regression test in pubrel's own tests:
`a_version_at_the_top_is_refused_not_overflowed` in `src/version.rs`.
