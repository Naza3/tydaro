#!/usr/bin/env python3
"""Select native builds without duplicating an open PR's merge validation."""

import json
import os
from pathlib import PurePosixPath
import re
import subprocess
from urllib.error import URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


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


def matches_push(pull_request: dict, repository: str, sha: str) -> bool:
    """A foreign, stale, closed or differently targeted PR cannot cover a push."""
    if not isinstance(pull_request, dict) or pull_request.get("state") != "open":
        return False
    head = pull_request.get("head")
    base = pull_request.get("base")
    if not isinstance(head, dict) or not isinstance(base, dict):
        return False
    head_repo = head.get("repo")
    base_repo = base.get("repo")
    return (
        isinstance(head_repo, dict)
        and isinstance(base_repo, dict)
        and head_repo.get("full_name") == repository
        and base_repo.get("full_name") == repository
        and head.get("ref") == "codex/dev"
        and base.get("ref") == "main"
        and head.get("sha") == sha
    )


def github_json(url: str, token: str):
    request = Request(url, headers={
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
    })
    with urlopen(request, timeout=10) as response:
        return json.load(response)


def matching_open_pull_request(
    event_name: str, event: dict, repository: str, token: str,
    api_url: str = "https://api.github.com",
) -> int | None:
    """Only defer codex/dev pushes when GitHub confirms a mergeable exact PR.

    pull_request events retain their normal merge-commit verification. Unknown
    API state deliberately causes a duplicate build instead of skipping checks.
    """
    if (
        event_name != "push"
        or event.get("ref") != "refs/heads/codex/dev"
        or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)
        or event.get("repository", {}).get("full_name") != repository
        or not token
    ):
        return None
    sha = event.get("after", "")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", sha) or set(sha) == {"0"}:
        return None
    query = urlencode({
        "state": "open", "base": "main",
        "head": f"{repository.split('/')[0]}:codex/dev", "per_page": 100,
    })
    endpoint = f"{api_url.rstrip('/')}/repos/{repository}/pulls"
    try:
        candidates = github_json(f"{endpoint}?{query}", token)
        if not isinstance(candidates, list):
            raise ValueError("Invalid pull request list")
        for candidate in candidates:
            if not matches_push(candidate, repository, sha):
                continue
            number = candidate.get("number")
            if not isinstance(number, int) or number <= 0:
                continue
            # The list API does not calculate mergeability. A conflicting PR
            # cannot run pull_request workflows, so query its current detail.
            detail = github_json(f"{endpoint}/{number}", token)
            if matches_push(detail, repository, sha) and detail.get("mergeable") is True:
                return number
    except (URLError, OSError, ValueError):
        # Do not print request objects or API responses, which may contain
        # credentials or unrelated repository information.
        print("Could not confirm PR coverage; native builds remain required.")
    return None


def select_build(event_name: str, event: dict) -> tuple[bool, str]:
    if not changes_require_build(event_name, event):
        return False, "Known documentation only; native jobs skipped."
    number = matching_open_pull_request(
        event_name, event, os.environ.get("GITHUB_REPOSITORY", ""),
        os.environ.get("GITHUB_TOKEN", ""),
        os.environ.get("GITHUB_API_URL", "https://api.github.com"),
    )
    if number is not None:
        return False, (
            f"This codex/dev push is covered by open PR #{number} at the same "
            "head commit; its pull_request run validates the merge commit."
        )
    return True, "Native Windows and Linux verification required."


def main() -> None:
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as event_file:
        event = json.load(event_file)
    build, reason = select_build(os.environ["GITHUB_EVENT_NAME"], event)
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"build={str(build).lower()}\n")
        output.write(f"reason={reason}\n")
    print(reason)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as output:
            output.write(f"### Native build selection\n\n{reason}\n")


if __name__ == "__main__":
    main()
