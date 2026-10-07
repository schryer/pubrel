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
the last release, under Cargo's flavour of semantic versioning, the one
`cargo` resolves `^` requirements by:

| Category | Meaning | From 1.0.0 | Below 1.0.0 |
|---|---|---|---|
| `changed`, `removed` | an already-published interface changed: commands, flags, output, exit codes, file formats, object shapes | major | minor |
| `added` | a new feature | minor | patch |
| `fixed`, `security` | the same functionality | patch | patch |
| `internal` | code changed, nothing users run did: a refactor, a crate moved | none | none |

Below 1.0.0, as Cargo reads `0.y.z`, `y` is the breaking part: `0.3.1` to
`0.4.0` breaks, and `0.3.1` to `0.3.2` does not, whatever it adds. So a
dependent's `^0.3` requirement takes every compatible release and no
breaking one.

A package never released before is first released at its manifest version,
raised to at least 0.1.0. `internal` changes never make a release on their
own; they wait for the next release something else makes, and are listed
in it.

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
  "install": "cargo install --locked --git https://github.com/schryer/publet --tag {tag} publet-cli",
  "key": "pub:sha2-256:m2smuezcqpjeyy6iofa2pjl47lgbwu254hohndyhs2do34fcrcma" }
```

`key` is the key a release must be signed by: the corpus's author, which
`pubrel init` records. `check` and `tag` verify that the package publet's
`identity` and `release` claims carry a valid signature by it, and that every
object they read hashes to its identifier; without `key` they warn and do
not check signatures.

A repository that releases more than one package lists them under
`packages`; `corpus` and `key` at the top are shared:

```json
{ "corpus": "corpus", "key": "pub:sha2-256:…",
  "packages": [
    { "name": "publet-cli", "package": "pkg.publet-cli", "tag": "PKG-PUBCLI-10-2026",
      "manifest": { "kind": "cargo-workspace", "path": "Cargo.toml" },
      "code": ["crates/", "bin/", "porcelain/", "Cargo.toml", "Cargo.lock"] },
    { "name": "publet-core", "package": "pkg.publet-core", "tag": "PKG-PUBCOR-10-2026",
      "manifest": { "kind": "cargo-package", "path": "crates/publet-core/Cargo.toml" },
      "code": ["crates/publet-core/"] } ] }
```

Each package has its own unreleased list, package publet, changelog (beside
its manifest: `crates/publet-core/CHANGELOG.md`) and tag prefix
(`publet-core-v0.1.1`; the root package keeps `v`; `tag-prefix` and
`changelog` override them). `add`, `next` and `prepare` take `--package
NAME`; `check`, `changelog` and `tag` act on every package. A changed path
belongs to every package whose `code` covers it, so a change to a crate
the workspace also ships is recorded in both lists. A workspace bump never
moves the pin of a crate released on its own, even when their versions
coincide; that crate's own release pins its new version in the workspace.

Manifest kinds: `cargo-workspace` (the workspace version and every internal
path dependency pinned at it), `cargo-package`, and `pyproject`. `command`
is optional; a library has none.

## Security checks

`release.json` may declare checks a release must pass, and facts to keep
beside them:

```json
"security": {
  "checks": [
    { "name": "advisories", "run": "cargo deny --locked check", "version": "cargo deny --version" },
    { "name": "supply chain", "run": "cargo vet --locked", "version": "cargo vet --version" }
  ],
  "facts": [
    { "name": "RustSec advisory database", "run": "git -C \"$(ls -d ~/.cargo/advisory-dbs/*/ | head -1)\" rev-parse HEAD" }
  ]
}
```

`pubrel prepare` runs every check before it changes anything, and cuts no
release if one fails. It records what ran in a `security` claim in the
package publet: each check's command, its tool and version, and the last
line the tool printed, beside each fact's value. The claim is signed with
the release, the changelog and the GitHub release list it, and `check` and
`tag` refuse a release whose record is missing or unsigned. What was
checked for a version is as verifiable as the version itself.

## API checks

A crate released on its own may set `"semver-checks": true` in its
`release.json` entry. Its public API is then compared with the one at its
last release's tag by
[`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks),
and may move only as far as the changes recorded since allow:

| recorded | the API may | level |
|---|---|---|
| `changed` or `removed` | break | major |
| `added` | grow, not break | minor |
| anything else | stay as it is | patch |

The level is what the change *is*, not the version it makes: below 1.0 a
break bumps the minor version, and is still a break. So a removed function
recorded as `fixed` fails `check` on its pull request, and again in
`prepare`, with the tool's findings and the `pubrel add` that would record
it. `check` runs it whenever the crate's code changes, and on a release
pull request; a crate never released has nothing to compare against, and
says so.

The baseline is a git tag, so CI needs the tags (`fetch-depth: 0`, as the
shared workflow checks out) and the tool: pass `semver-checks-version`.

## CI

Every package calls the same workflow:

```yaml
release:
  uses: schryer/pubrel/.github/workflows/release.yml@vX.Y.Z
  with: { pubrel-version: vX.Y.Z }   # add semver-checks-version: 0.51.0 for API checks
  permissions: { contents: write }
```

On a pull request it runs `pubrel check`: a change to a `code` path must
record a row, and a release must be exactly the bump its changes require,
published. On main it runs `pubrel tag`, which tags a published version and
creates the GitHub release. CI never signs anything: a release is signed
when its package publet is published, by whoever cut it.

## Requirements

Cutting a release (`pubrel prepare`) drives a released `pub` (^0.1) to
build and publish the package publet, which signs it; that happens on the
machine of whoever cuts it. Everything else -- `next`, `check`, `tag`,
`changelog` -- reads the package publet and verifies its signatures itself,
with [`publet-core`](https://github.com/schryer/publet/tree/main/crates/publet-core),
so CI needs only `pubrel`, plus `git` and `gh`. Install:

```sh
cargo install --locked --git https://github.com/schryer/publet --tag v0.1.1 publet-cli   # to cut releases
cargo install --locked --git https://github.com/schryer/pubrel --tag vX.Y.Z pubrel
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
are resolved from `PUBREL_BIN_DIR` and `PUB_BIN_DIR`. The shared testing
setup -- the plugin those fixtures and the common steps come from,
`pubkit.mk`, the hash-locked pins, and the CI workflow -- is
[pubkit](https://github.com/schryer/pubkit)'s; `make sync-check` fails if
the managed copies drift. pubrel's own CI calls its own release workflow
from the checkout, so it is held to the rules it provides.

## Licence

MIT.
