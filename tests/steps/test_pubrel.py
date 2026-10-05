"""Steps for every pubrel feature."""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/add.feature")
scenarios("../features/next.feature")
scenarios("../features/check.feature")
scenarios("../features/prepare.feature")
scenarios("../features/tag.feature")
scenarios("../features/changelog.feature")
scenarios("../features/verify.feature")
scenarios("../features/multi.feature")


# --- givens ------------------------------------------------------------------


@given(parsers.parse('a released-never {kind} package at version "{version}"'),
       target_fixture="pkg")
def never_released(make_package, kind: str, version: str):
    return make_package(kind, version)


@given(parsers.parse('a {kind} package released at version "{version}"'), target_fixture="pkg")
def released(make_package, kind: str, version: str):
    pkg = make_package(kind, version)
    pkg.add("added", "the first release")
    pkg.commit("record the first release's changes")
    pkg.git("push", "-q", "origin", "main")
    pkg.release()
    return pkg


@given(parsers.parse('the unreleased changes "{categories}"'))
def unreleased_changes(pkg, categories: str):
    for category in categories.split(","):
        pkg.add(category, f"a {category} change")


@given(parsers.parse('the unreleased changes "{categories}" pushed to main'))
def unreleased_pushed(pkg, categories: str):
    unreleased_changes(pkg, categories)
    pkg.commit("record changes")
    pkg.git("push", "-q", "origin", "main")


@given("an uncommitted edit")
def uncommitted(pkg):
    (pkg.root / pkg.source).write_text("// edited\n")


@given(parsers.parse('it is released again with "{category}" "{change}"'))
def released_again(pkg, category: str, change: str):
    pkg.add(category, change)
    pkg.commit("record a change")
    pkg.git("push", "-q", "origin", "main")
    pkg.release()


@given(parsers.parse('the tag "{tag}" already exists'))
def tag_exists(pkg, tag: str):
    pkg.git("tag", tag)


# --- whens -------------------------------------------------------------------


@when(parsers.parse('I run pubrel add "{category}" "{change}"'), target_fixture="result")
def run_add(pkg, category: str, change: str):
    return pkg.pubrel_run("add", category, change)


@when("I run pubrel next", target_fixture="result")
def run_next(pkg):
    return pkg.pubrel_run("next")


@when("I run pubrel prepare", target_fixture="result")
def run_prepare(pkg):
    return pkg.pubrel_run("prepare")


@when("I run pubrel tag", target_fixture="result")
def run_tag(pkg):
    return pkg.pubrel_run("tag")


@when("I run pubrel changelog", target_fixture="result")
def run_changelog(pkg):
    return pkg.pubrel_run("changelog")


@when(parsers.parse('a branch changes "{path}"'))
def branch_changes(pkg, path: str):
    pkg.git("checkout", "-q", "-b", "feature")
    target = pkg.root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("// changed\n")
    pkg.commit("a change")


@when(parsers.parse('a branch changes "{path}" and records "{category}" "{change}"'))
def branch_changes_and_records(pkg, path: str, category: str, change: str):
    branch_changes(pkg, path)
    pkg.add(category, change)
    pkg.commit("record it")


@when(parsers.parse('a branch records "{category}" "{change}" and is released'))
def branch_released(pkg, category: str, change: str):
    # As merged: the change and its record land on main; the release PR is
    # then cut from main and checked against it.
    pkg.add(category, change)
    pkg.commit("record a change")
    pkg.git("push", "-q", "origin", "main")
    pkg.ok(str(pkg.pubrel), "prepare", "--no-pr")


@when(parsers.parse('the branch\'s manifest is set to "{version}"'))
def set_manifest(pkg, version: str):
    path = pkg.root / pkg.manifest
    text = path.read_text().replace(f'version = "{pkg.manifest_version()}"',
                                    f'version = "{version}"', 1)
    path.write_text(text)
    pkg.commit("tamper with the version")


@when("pub is not available")
def no_pub(pkg):
    pkg.without_pub()


@when("the release's signatures are removed")
def unsign(pkg):
    """Delete every signature over the package publet's claims."""
    targets = {pkg.locked(f"pkg.demo#{part}") for part in ("identity", "release")}
    for path in pkg.objects().glob("*.cbor"):
        obj = json.loads(pkg.pub("read", "--json", str(path)).stdout)["object"]
        if obj["type"] == "sig" and obj["body"]["target"] in targets:
            path.unlink()
    pkg.commit("remove the release's signatures")


@when("release.json names the identity claim as its key")
def wrong_key(pkg):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config["key"] = pkg.locked("pkg.demo#identity")
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("name the wrong key")


@when("release.json names no key")
def no_key(pkg):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    del config["key"]
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("name no key")


@when("the published identity claim is altered")
def alter(pkg):
    # Same length, so the bytes stay canonical CBOR and only the identifier
    # gives the alteration away.
    path = pkg.object_file(pkg.locked("pkg.demo#identity"))
    data = path.read_bytes()
    assert b"0.1.1" in data
    path.write_bytes(data.replace(b"0.1.1", b"0.1.2"))
    pkg.commit("alter the published identity")


@when("I run pubrel check against main", target_fixture="result")
def run_check(pkg):
    return pkg.pubrel_run("check", "origin/main")


# --- thens -------------------------------------------------------------------


@then(parsers.parse('the unreleased list holds "{category}" "{change}"'))
def holds(pkg, category: str, change: str):
    assert {"category": category, "change": change} in pkg.unreleased()


@then("the unreleased list is empty")
def empty(pkg):
    assert pkg.unreleased() == []


@then(parsers.parse('the manifest states "{version}"'))
def manifest_states(pkg, version: str):
    assert pkg.manifest_version() == version


@then(parsers.parse('the manifest pins its internal dependency at "{version}"'))
def internal_pin(pkg, version: str):
    text = (pkg.root / pkg.manifest).read_text()
    assert f'core = {{ path = "crates/core", version = "{version}" }}' in text, text


@then(parsers.parse('the published release record states "{version}" with "{category}"'))
def record_states(pkg, version: str, category: str):
    body = pkg.published("pkg.demo#release")["body"]
    assert f"demo {version} was released" in body["content"], body["content"]
    assert category in [row[0] for row in body["data"]["rows"]]


@then(parsers.parse('CHANGELOG.md has a section "{heading}"'))
def changelog_section(pkg, heading: str):
    assert heading in (pkg.root / "CHANGELOG.md").read_text()


@then(parsers.parse('a pull request "{title}" was requested from "{branch}"'))
def pr_requested(pkg, title: str, branch: str):
    calls = pkg.gh_calls()
    assert any(c[:2] == ["pr", "create"] and title in c for c in calls), calls
    assert pkg.git("rev-parse", "--abbrev-ref", "HEAD") == branch
    assert branch in pkg.git("ls-remote", "--heads", "origin")


@then(parsers.parse('the tag "{tag}" exists and names the package publet'))
def tag_names_package(pkg, tag: str):
    message = pkg.git("tag", "-l", "--format=%(contents)", tag)
    assert "package pkg.demo [PKG-DEMOPK-10-2026]" in message, message
    assert tag in pkg.git("ls-remote", "--tags", "origin")


@then(parsers.parse('a GitHub release "{tag}" was requested'))
def release_requested(pkg, tag: str):
    assert any(c[:3] == ["release", "create", tag] for c in pkg.gh_calls()), pkg.gh_calls()


@then("no tag exists")
def no_tag(pkg):
    assert pkg.git("tag", "-l") == ""


@then(parsers.parse('CHANGELOG.md lists "{newer}" before "{older}"'))
def newest_first(pkg, newer: str, older: str):
    text = (pkg.root / "CHANGELOG.md").read_text()
    assert text.index(f"## {newer}") < text.index(f"## {older}"), text


@then(parsers.parse('CHANGELOG.md has "{entry}" under "{heading}"'))
def under_heading(pkg, entry: str, heading: str):
    text = (pkg.root / "CHANGELOG.md").read_text()
    section = text.split(heading, 1)[1]
    assert f"- {entry}" in section.split("##", 1)[0], text


# --- several packages --------------------------------------------------------


@given(parsers.parse('a workspace whose "{a}" and "{b}" are released at version "{version}"'),
       target_fixture="pkg")
def multi_released(make_package, a: str, b: str, version: str):
    pkg = make_package("multi", version)
    for name in (a, b):
        pkg.add("added", f"the first release of {name}", package=name)
        pkg.commit(f"record {name}'s first release")
        pkg.git("push", "-q", "origin", "main")
        pkg.release(package=name)
    return pkg


@when(parsers.parse('"{name}" records "{category}" "{change}" and is released'))
def multi_release(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")
    pkg.git("push", "-q", "origin", "main")
    pkg.release(package=name)


@when(parsers.parse('"{name}" records "{category}" "{change}" and its release is cut'))
def multi_cut(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")
    pkg.git("push", "-q", "origin", "main")
    pkg.cut(package=name)


@when(parsers.parse('"{name}" records "{category}" "{change}" on main'))
def multi_record(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")


@when(parsers.parse('a branch changes "{path}" and records "{category}" "{change}" for "{name}"'))
def multi_branch(pkg, path: str, category: str, change: str, name: str):
    branch_changes(pkg, path)
    pkg.add(category, change, package=name)
    pkg.commit("record it")


@when(parsers.parse('it also records "{category}" "{change}" for "{name}"'))
def multi_also(pkg, category: str, change: str, name: str):
    pkg.add(category, change, package=name)
    pkg.commit("record it too")


@when(parsers.parse('I run pubrel add "{category}" "{change}" without naming a package'),
      target_fixture="result")
def multi_add_unnamed(pkg, category: str, change: str):
    return pkg.pubrel_run("add", category, change)


@then(parsers.parse('the release branch is "{branch}"'))
def release_branch(pkg, branch: str):
    assert pkg.last_branch == branch, pkg.last_branch


@then(parsers.parse('"{manifest}" states "{version}"'))
def manifest_at(pkg, manifest: str, version: str):
    assert pkg.manifest_version(manifest) == version


@then(parsers.parse('the workspace states "{version}"'))
def workspace_states(pkg, version: str):
    assert pkg.manifest_version("Cargo.toml") == version


@then(parsers.parse('the workspace pins "{path}" at "{version}"'))
def workspace_pins(pkg, path: str, version: str):
    assert pkg.pinned(path) == version, (pkg.root / "Cargo.toml").read_text()


@then(parsers.parse('the tag "{tag}" exists'))
def tag_present(pkg, tag: str):
    assert pkg.git("tag", "-l", tag) == tag


@then(parsers.parse('"{path}" names the package publet "{slug}"'))
def changelog_names(pkg, path: str, slug: str):
    text = (pkg.root / path).read_text()
    assert f"package publet `{slug}`" in text, text
    assert "## 0.1.0" in text, text
