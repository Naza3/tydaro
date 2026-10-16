#!/usr/bin/env python3
"""Check the enforced crate and composition boundaries without building Rust.

Uses Cargo's manifest interpretation, not a second TOML parser. The source check
is deliberately limited to explicit Rust module paths; it is not a Rust compiler
or a claim that all architectural properties can be checked statically.
"""
from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess
import sys


def code_only(source: str) -> str:
    """Blank comments and string literals, retaining character offsets."""
    chars = list(source)
    i = 0
    while i < len(source):
        start = i
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end
        elif source.startswith("/*", i):
            i += 2
            depth = 1
            while i < len(source) and depth:
                if source.startswith("/*", i):
                    depth += 1
                    i += 2
                elif source.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        else:
            raw = re.match(r'r(#+)?"', source[i:]) if source[i] == "r" else None
            if raw:
                closing = '"' + (raw.group(1) or "")
                end = source.find(closing, i + raw.end())
                i = len(source) if end < 0 else end + len(closing)
            elif source[i] == '"':
                i += 1
                while i < len(source):
                    if source[i] == "\\":
                        i += 2
                    elif source[i] == '"':
                        i += 1
                        break
                    else:
                        i += 1
            elif source[i] == "'" and re.match(r"'(?:\\.|[^'\\\n])'", source[i:]):
                i += len(re.match(r"'(?:\\.|[^'\\\n])'", source[i:]).group())
            else:
                i += 1
                continue
        for position in range(start, min(i, len(chars))):
            if chars[position] != "\n":
                chars[position] = " "
    return "".join(chars)


def production_code(source: str) -> str:
    code = code_only(source)
    # These are the two test-only cfg forms used in this repository. Other cfg
    # forms are inspected conservatively, so platform branches remain covered.
    pattern = r'#\[cfg\((?:test|feature\s*=\s*"test-support")\)\]'
    for match in re.finditer(pattern, source):
        start, after_attribute = match.span()
        if not code[start:].startswith("#[cfg("):
            continue
        brace = code.find("{", after_attribute)
        semicolon = code.find(";", after_attribute)
        if semicolon >= 0 and (brace < 0 or semicolon < brace):
            end = semicolon + 1
        elif brace >= 0:
            depth, end = 1, brace + 1
            while end < len(code) and depth:
                depth += (code[end] == "{") - (code[end] == "}")
                end += 1
        else:
            end = len(code)
        code = code[:start] + " " * (end - start) + code[end:]
    return code


def crate_modules(source: str) -> set[str]:
    code = production_code(source)
    # The checked file is a root module, so `super` also refers to the crate root.
    modules = set(re.findall(r"\b(?:crate|super)::(\w+)", code))
    for match in re.finditer(r"\buse\s+(?:crate|super)::\{", code):
        depth, part = 0, ""
        for char in code[match.end():]:
            if depth == 0 and char in ",}":
                root = re.match(r"\s*(\w+)", part)
                if root:
                    modules.add(root.group(1))
                if char == "}":
                    break
                part = ""
                continue
            depth += (char == "{") - (char == "}")
            part += char
    return modules


def self_test() -> None:
    fixture = '''
// crate::desktop must be ignored here.
const NOTE: &str = r#"crate::component_gallery"#;
use crate::{storage::{self, SavedPanel}, market::Symbol};
fn render() { crate::transfer::finish(); }
#[cfg(test)]
mod tests { use crate::desktop; }
#[cfg(feature = "test-support")]
fn smoke() { crate::component_gallery::open(); }
'''
    assert crate_modules(fixture) == {"storage", "market", "transfer"}
    assert crate_modules("/* nested /* crate::app */ */ use crate::desktop;") == {"desktop"}
    assert crate_modules("fn x() { let ch = '}'; crate::app::init(); }") == {"app"}
    assert crate_modules("use super::{desktop::Desktop, storage};") == {"desktop", "storage"}


def check(root: Path) -> list[str]:
    process = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, encoding="utf-8",
    )
    if process.returncode:
        raise RuntimeError(process.stderr.strip())
    metadata = json.loads(process.stdout)
    members = set(metadata["workspace_members"])
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in members}
    failures: list[str] = []

    def dependencies(name: str, seen: set[str] | None = None) -> set[str]:
        seen = set() if seen is None else seen
        if name in seen or name not in packages:
            return set()
        seen.add(name)
        direct = {dependency["name"] for dependency in packages[name]["dependencies"]}
        result = set(direct)
        for dependency in direct:
            result.update(dependencies(dependency, seen))
        return result

    for name in ("tydaro", "tydaro-core", "tydaro-ui"):
        if name not in packages:
            failures.append(f"Missing required workspace package: {name}")
    core = dependencies("tydaro-core")
    invalid_core = {name for name in core if name.startswith("gpui") or name in {"tydaro", "tydaro-ui"}}
    if invalid_core:
        failures.append(f"tydaro-core depends on presentation packages: {sorted(invalid_core)}")
    invalid_ui = dependencies("tydaro-ui") & {"tydaro", "tydaro-core"}
    if invalid_ui:
        failures.append(f"tydaro-ui depends on application/domain packages: {sorted(invalid_ui)}")

    workspace_path = root / "src" / "workspace.rs"
    forbidden = {"desktop", "desktop_state", "component_gallery", "gallery_preferences", "board_bar", "app"}
    bad_modules = crate_modules(workspace_path.read_text(encoding="utf-8-sig")) & forbidden
    if bad_modules:
        failures.append(f"workspace.rs references its composition host: {sorted(bad_modules)}")
    return failures


def main() -> int:
    self_test()
    if "--self-test" in sys.argv:
        print("Architecture source scanner self-test passed.")
        return 0
    failures = check(Path(__file__).resolve().parent.parent)
    if failures:
        for failure in failures:
            print(f"ERROR: {failure}", file=sys.stderr)
        return 1
    print("Architecture boundaries passed: core, UI, and workspace composition.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, AssertionError) as error:
        print(f"Architecture check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
