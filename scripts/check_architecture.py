#!/usr/bin/env python3
"""Check the enforced crate and composition boundaries without building Rust.

Uses Cargo's manifest interpretation, not a second TOML parser. The source check
covers explicit crate/self/super paths in conventional file and inline modules.
It does not expand macros, resolve aliases or custom #[path] declarations, and
is not a Rust compiler or a complete check of architectural properties.
"""
from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile


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


def production_cfg(expression: str) -> bool | None:
    """Evaluate only test flags; None keeps unknown/platform branches covered."""
    tokens = re.findall(r'\w+|"(?:\\.|[^"\\])*"|[(),=]', expression)
    position = 0

    def parse() -> bool | None:
        nonlocal position
        if position >= len(tokens):
            return None
        name = tokens[position]
        position += 1
        if position < len(tokens) and tokens[position] == "=":
            position += 1
            value = tokens[position] if position < len(tokens) else ""
            position += 1
            return False if (name, value) == ("feature", '"test-support"') else None
        if position < len(tokens) and tokens[position] == "(":
            position += 1
            values = []
            while position < len(tokens) and tokens[position] != ")":
                values.append(parse())
                if position < len(tokens) and tokens[position] == ",":
                    position += 1
                elif position < len(tokens) and tokens[position] != ")":
                    return None
            position += 1
            if name == "any":
                return True if True in values else (None if None in values else False)
            if name == "all":
                return False if False in values else (None if None in values else True)
            if name == "not" and len(values) == 1 and values[0] is not None:
                return not values[0]
            return None
        return False if name == "test" else None

    result = parse()
    return result if position == len(tokens) else None


def item_end(code: str, start: int) -> int:
    """Find the end of a cfg-gated item without entering its nested groups."""
    item = re.search(r"\b(use|const|static|type|fn|mod|impl|trait|struct|enum|union)\b", code[start:])
    semicolon_item = item is not None and item.group(1) in {"use", "const", "static", "type"}
    stack = []
    for position in range(start, len(code)):
        char = code[position]
        if char in "([{":
            stack.append(char)
        elif char in ")]}" and stack:
            opening = stack.pop()
            if opening == "{" and not stack and not semicolon_item:
                return position + 1
        elif char == ";" and not stack:
            return position + 1
    return len(code)


def production_code(source: str) -> str:
    code = code_only(source)
    pattern = r'#\s*\[\s*cfg\s*\((.*?)\)\s*\]'
    for match in re.finditer(pattern, source, re.DOTALL):
        start, after_attribute = match.span()
        if code[start] != "#" or production_cfg(match.group(1)) is not False:
            continue
        end = item_end(code, after_attribute)
        code = code[:start] + re.sub(r"[^\n]", " ", code[start:end]) + code[end:]
    return code


def inline_modules(code: str) -> list[tuple[int, int, str]]:
    """Locate inline modules so relative paths have their actual lexical owner."""
    openings = {
        match.end() - 1: match.group(1)
        for match in re.finditer(r"\bmod\s+(\w+)\s*\{", code)
    }
    stack = []
    modules = []
    for position, char in enumerate(code):
        if char == "{":
            stack.append(position)
        elif char == "}" and stack:
            start = stack.pop()
            if start in openings:
                modules.append((start, position, openings[start]))
    return sorted(modules)


def use_paths(statement: str) -> list[tuple[str, ...]]:
    """Expand a Rust use tree, preserving crate/self/super prefixes and groups."""
    tokens = re.findall(r"(?:r#)?[A-Za-z_]\w*|::|[{},;*]", statement)
    position = 0
    paths = []

    def tree(prefix: tuple[str, ...] = ()) -> None:
        nonlocal position
        path = list(prefix)
        while position < len(tokens):
            token = tokens[position]
            if token in {",", "}", ";"}:
                break
            position += 1
            if token == "::":
                continue
            if token == "{":
                while position < len(tokens) and tokens[position] != "}":
                    previous = position
                    tree(tuple(path))
                    if position < len(tokens) and tokens[position] == ",":
                        position += 1
                    elif position == previous:
                        break  # Malformed Rust is left to the compiler.
                if position < len(tokens):
                    position += 1
                return
            if token == "as":
                position += 1  # The local alias is not a module path.
                break
            path.append(token.removeprefix("r#"))
        paths.append(tuple(path))

    tree()
    return paths


def path_root(path: tuple[str, ...], owner: tuple[str, ...]) -> str | None:
    if not path or path[0] not in {"crate", "self", "super"}:
        return None  # Unqualified names/aliases need Rust name resolution.
    resolved = [] if path[0] == "crate" else list(owner)
    for index, segment in enumerate(path):
        if index == 0 and segment == "crate":
            continue
        if segment == "super":
            if not resolved:
                return None  # An invalid Rust path; the compiler reports it.
            resolved.pop()
        elif segment != "self":
            resolved.append(segment)
    return resolved[0] if resolved else None


def crate_modules(source: str, owner: tuple[str, ...] = ("workspace",)) -> set[str]:
    code = production_code(source)
    nested = inline_modules(code)
    modules = set()

    def inspect(path: tuple[str, ...], position: int) -> None:
        context = owner + tuple(name for start, end, name in nested if start < position < end)
        root = path_root(path, context)
        if root is not None:
            modules.add(root)

    # Import trees require their own expansion. Mask them before inspecting
    # expression/type paths, so nested `self` branches cannot lose their prefix.
    for match in re.finditer(r"\buse\s+([^;]+);", code):
        for path in use_paths(match.group(1)):
            inspect(path, match.start())
        code = code[:match.start()] + re.sub(r"[^\n]", " ", match.group()) + code[match.end():]
    pattern = r"\b(?:crate|self|super)\s*::\s*(?:r#)?\w+(?:\s*::\s*(?:r#)?\w+)*"
    for match in re.finditer(pattern, code):
        path = tuple(part.strip().removeprefix("r#") for part in match.group().split("::"))
        inspect(path, match.start())
    return modules


FORBIDDEN_WORKSPACE_ROOTS = {
    "app", "pages", "gallery", "test_support",
    # Preserve the boundary if a former top-level module is reintroduced.
    "desktop", "desktop_state", "component_gallery", "gallery_preferences", "board_bar",
}


def workspace_failures(root: Path) -> list[str]:
    directory = root / "src" / "workspace"
    if not (directory / "mod.rs").is_file():
        return ["Missing workspace module root: src/workspace/mod.rs"]
    failures = []
    for path in sorted(directory.rglob("*.rs")):
        relative = path.relative_to(root / "src")
        if any(part in {"tests", "test_support"} for part in relative.parts[:-1]):
            continue
        if path.name in {"tests.rs", "test_support.rs"}:
            continue
        owner = relative.with_suffix("").parts
        if owner[-1] == "mod":
            owner = owner[:-1]
        bad = crate_modules(path.read_text(encoding="utf-8-sig"), owner) & FORBIDDEN_WORKSPACE_ROOTS
        if bad:
            failures.append(f"{path.relative_to(root).as_posix()} references its composition host: {sorted(bad)}")
    return failures


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
    operations = ("workspace", "operations")
    assert crate_modules("use super::{board_bar::BoardBar, panel};", operations) == {"workspace"}
    assert crate_modules("use self::app::Local; super::panel::run();", operations) == {"workspace"}
    assert crate_modules("use super::super::{app::Shell, pages::{self, Watch}};", operations) == {"app", "pages"}
    assert crate_modules("use {crate::gallery::Gallery as Preview, super::{super::pages, panel}};", operations) == {"gallery", "pages", "workspace"}
    assert crate_modules("fn run() { super::super::app::run(); }", operations) == {"app"}
    assert crate_modules("mod child { use super::super::panel; }", operations) == {"workspace"}
    assert crate_modules("mod child { use super::super::super::app; }", operations) == {"app"}
    assert crate_modules("use crate::{workspace::{self, board_bar}, adapters::{self, layout}};") == {"workspace", "adapters"}
    assert crate_modules('''
#[cfg(any(test, feature = "test-support"))]
use crate::{app, pages};
#[cfg(all(target_os = "windows", test))]
const NOTE: [u8; 1] = [crate::gallery::VALUE];
#[cfg(feature = "test-support")]
mod helper { fn inspect() { crate::test_support::run(); } }
use super::panel;
''', operations) == {"workspace"}
    assert crate_modules('''
#[cfg(not(test))]
fn production() { crate::app::run(); }
#[cfg(any(test, target_os = "windows"))]
fn platform() { crate::pages::run(); }
''', operations) == {"app", "pages"}

    # Test the actual recursive filesystem entry point, not only parser helpers.
    with tempfile.TemporaryDirectory(prefix="tydaro-architecture-") as temporary:
        root = Path(temporary)
        sources = {
            "mod.rs": "mod nested; use crate::adapters::layout;",
            "nested/mod.rs": "mod commands; use super::board_bar;",
            "nested/commands.rs": "use super::super::{panel, board_bar};",
            "tests.rs": "use crate::app;",
            "nested/tests.rs": "use crate::pages;",
            "test_support.rs": "use crate::gallery;",
            "test_support/scenarios.rs": "use crate::app;",
            "nested/tests/fixtures.rs": "use crate::test_support;",
        }
        for name, source in sources.items():
            path = root / "src" / "workspace" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(source, encoding="utf-8")
        assert workspace_failures(root) == []
        nested = root / "src" / "workspace" / "nested" / "commands.rs"
        nested.write_text("use super::super::super::{app::Shell, pages::Watch};", encoding="utf-8")
        failures = workspace_failures(root)
        assert len(failures) == 1 and "src/workspace/nested/commands.rs" in failures[0]
        assert "['app', 'pages']" in failures[0]
        nested.write_text("use crate::gallery::Gallery;", encoding="utf-8")
        assert "['gallery']" in workspace_failures(root)[0]


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

    failures.extend(workspace_failures(root))
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
