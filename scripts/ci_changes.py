#!/usr/bin/env python3
"""Skip native builds only when every changed path is known documentation."""

import json
import os
from pathlib import PurePosixPath
import re
import subprocess


def documentation_only(paths: list[str]) -> bool:
    if not paths:
        return False
    root_documents = {"README.md", "CONTRIBUTING.md", "AGENTS.md"}
    for path in paths:
        if path in root_documents or path == ".github/pull_request_template.md":
            continue
        parsed = PurePosixPath(path)
        if (
            parsed.parts[0] == "docs"
            and parsed.suffix.lower() in {".md", ".png", ".jpg", ".jpeg", ".svg"}
        ):
            continue
        return False
    return True


def changes_require_build(event_name: str, event: dict) -> bool:
    if event_name == "pull_request":
        base = event["pull_request"]["base"]["sha"]
    elif event_name == "push":
        base = event.get("before", "")
    else:
        return True  # A manual run always builds, including a docs-only commit.
    if not re.fullmatch(r"[0-9a-fA-F]{40}", base) or set(base) == {"0"}:
        return True
    try:
        # Checkout fetches full history, including the PR merge's base. Disable
        # rename detection: moving source to docs must count its deleted path.
        diff = subprocess.check_output(
            ["git", "diff", "--name-only", "--no-renames", "-z", base, "HEAD"],
            stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError:
        return True  # An unavailable history must never skip verification.
    paths = [path.decode("utf-8", errors="replace") for path in diff.split(b"\0") if path]
    return not documentation_only(paths)


def main() -> None:
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as event_file:
        event = json.load(event_file)
    build = changes_require_build(os.environ["GITHUB_EVENT_NAME"], event)
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"build={str(build).lower()}\n")
    print("Native build required" if build else "Documentation only; native jobs skipped")


if __name__ == "__main__":
    main()
