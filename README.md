# pubrel

Version a package by what changed, and record each release as a version of
a publet.

A release of a package is a version of its **package publet**: `pkg.<name>`
in the package's own corpus, whose `identity` claim states the version, the
`vX.Y.Z` tag, and the commit, and whose `release` claim lists what changed.
The publet's lineage is the release history; `CHANGELOG.md` is generated
from it. One tool, one record, one workflow, for every package -- Rust or
Python.

## The rules

The version follows mechanically from the categories of what changed since
the last release:

| Category | Meaning | Bump |
|---|---|---|
| `changed`, `removed` | an already-published interface changed: commands, flags, output, exit codes, file formats, object shapes | major |
| `added` | a new feature | minor |
| `fixed`, `security` | the same functionality | patch |

A package never released before is first released at its manifest version,
raised to at least 0.1.0.

## Using it

```sh
pubrel init                                  # release.json and an empty unreleased list
pubrel add added "a new subcommand"          # every PR that changes code records a row
pubrel next                                  # the version the rows imply
pubrel prepare                               # on a clean main: cut a release PR
```

`pubrel prepare` moves the unreleased rows into the package publet, sets the
manifest version, publishes the package publet with the corpus's key,
regenerates `CHANGELOG.md`, and opens a `release/vX.Y.Z` pull request.
Merging it is the release: CI tags it.

`release.json` at the repository root says where everything is:

```json
{ "name": "publet-cli", "package": "pkg.publet-cli", "tag": "PKG-PUBCLI-10-2026",
  "corpus": "corpus",
  "manifest": { "kind": "cargo-workspace", "path": "Cargo.toml" },
  "code": ["crates/", "bin/", "porcelain/", "Cargo.toml", "Cargo.lock"],
  "command": { "name": "pub", "check": "pub --version" },
  "install": "cargo install --locked --git https://github.com/schryer/publet --tag {tag} publet-cli" }
```

Manifest kinds: `cargo-workspace` (the workspace version and every internal
path dependency pinned at it), `cargo-package`, and `pyproject`. `command`
is optional; a library has none.

## CI

Every package calls the same workflow:

```yaml
release:
  uses: schryer/pubrel/.github/workflows/release.yml@v0.1.0
  with: { pub-version: v0.1.1, pubrel-version: v0.1.0 }
  permissions: { contents: write }
```

On a pull request it runs `pubrel check`: a change to a `code` path must
record a row, and a release must be exactly the bump its changes require,
published. On main it runs `pubrel tag`, which tags a published version and
creates the GitHub release. CI never signs anything: a release is signed
when its package publet is published, by whoever cut it.

## Requirements

`pubrel` drives a released `pub` (^0.1) for everything that touches
publets -- building, publishing, and reading the package publet back -- and
`git` and `gh` for the rest. Install:

```sh
cargo install --locked --git https://github.com/schryer/publet --tag v0.1.1 publet-cli
cargo install --locked --git https://github.com/schryer/pubrel --tag v0.1.0 pubrel
```

## Developing

```sh
make venv          # hash-locked Python environment for the functional suite
make check         # fmt, clippy, unit tests, docs, and the Gherkin suite
```

Testing is split as in publet: Rust unit tests for invariants with no
user-visible surface (the bump arithmetic, manifest editing); a Gherkin
suite (`tests/features`) for every behaviour, driving the built binary
against throwaway repositories with a real `pub` and a fake `gh`. Binaries
are resolved from `PUBREL_BIN_DIR` and `PUB_BIN_DIR`. pubrel's own CI calls
its own release workflow from the checkout, so it is held to the rules it
provides.

## Licence

MIT.
