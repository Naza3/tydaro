#!/usr/bin/env python3
"""Check documentation skips and conservative push/PR build selection."""

from copy import deepcopy
import os
import subprocess
import unittest
from unittest.mock import patch
from urllib.error import URLError

from ci_changes import (
    changes_require_build, documentation_only, matching_open_pull_request, select_build,
)


class ChangePolicyTests(unittest.TestCase):
    def test_documentation_allowlist(self):
        self.assertTrue(documentation_only(["README.md", "docs/architecture.md", "docs/preview.png"]))
        self.assertTrue(documentation_only(["AGENTS.md", "CONTRIBUTING.md"]))

    def test_unknown_or_build_paths_require_build(self):
        for path in (
            "src/main.rs", "Cargo.lock", "Cargo.toml", "rust-toolchain.toml",
            "scripts/ci_changes.py", ".github/workflows/build.yml", ".githooks/pre-push",
            "docs/generator.py", "assets/logo.svg", "new-feature.md",
        ):
            with self.subTest(path=path):
                self.assertFalse(documentation_only(["README.md", path]))

    def test_empty_change_set_requires_build(self):
        self.assertFalse(documentation_only([]))

    def test_manual_run_requires_build(self):
        self.assertTrue(changes_require_build("workflow_dispatch", {}))

    def test_new_branch_or_invalid_base_requires_build(self):
        for base in ("0" * 40, "", "--bad-option"):
            self.assertTrue(changes_require_build("push", {"before": base}))

    @patch("ci_changes.subprocess.check_output")
    def test_pr_uses_base_and_docs_skip(self, git):
        git.return_value = b"docs/user guide.md\0README.md\0"
        event = {"pull_request": {"base": {"sha": "a" * 40}}}
        self.assertFalse(changes_require_build("pull_request", event))
        self.assertEqual(git.call_args.args[0], [
            "git", "diff", "--name-only", "--no-renames", "-z", "a" * 40, "HEAD",
        ])

    @patch("ci_changes.subprocess.check_output")
    def test_move_out_of_source_requires_build(self, git):
        git.return_value = b"src/main.rs\0docs/example.md\0"
        self.assertTrue(changes_require_build("push", {"before": "a" * 40}))

    @patch("ci_changes.subprocess.check_output")
    def test_unavailable_base_requires_build(self, git):
        git.side_effect = subprocess.CalledProcessError(128, "git")
        self.assertTrue(changes_require_build("push", {"before": "a" * 40}))


class PushPullRequestPolicyTests(unittest.TestCase):
    def setUp(self):
        self.repository = "Naza3/tydaro"
        self.sha = "b" * 40
        self.event = {
            "ref": "refs/heads/codex/dev", "after": self.sha,
            "repository": {"full_name": self.repository},
        }
        self.pull_request = {
            "number": 7, "state": "open", "mergeable": True,
            "head": {
                "ref": "codex/dev", "sha": self.sha,
                "repo": {"full_name": self.repository},
            },
            "base": {"ref": "main", "repo": {"full_name": self.repository}},
        }

    def coverage(self, event_name="push", event=None):
        return matching_open_pull_request(
            event_name, self.event if event is None else event,
            self.repository, "test-token",
        )

    @patch("ci_changes.github_json")
    def test_same_repo_exact_mergeable_pr_covers_push(self, api):
        api.side_effect = [[self.pull_request], self.pull_request]
        self.assertEqual(self.coverage(), 7)
        self.assertIn("head=Naza3%3Acodex%2Fdev", api.call_args_list[0].args[0])
        self.assertEqual(api.call_args_list[1].args[0],
                         "https://api.github.com/repos/Naza3/tydaro/pulls/7")

    @patch("ci_changes.github_json")
    def test_no_open_pr_builds_push(self, api):
        api.return_value = []
        self.assertIsNone(self.coverage())

    @patch("ci_changes.github_json")
    def test_foreign_stale_closed_and_other_branches_do_not_cover_push(self, api):
        candidates = []
        for side, field, value in (
            ("head", "repo", {"full_name": "someone/tydaro"}),
            ("base", "repo", {"full_name": "someone/tydaro"}),
            ("head", "sha", "c" * 40),
            ("head", "ref", "another-branch"),
            ("base", "ref", "another-base"),
        ):
            candidate = deepcopy(self.pull_request)
            candidate[side][field] = value
            candidates.append(candidate)
        candidate = deepcopy(self.pull_request)
        candidate["state"] = "closed"
        candidates.append(candidate)
        api.return_value = candidates
        self.assertIsNone(self.coverage())
        api.assert_called_once()

    @patch("ci_changes.github_json")
    def test_conflicts_and_unknown_mergeability_build_push(self, api):
        for mergeable in (False, None):
            with self.subTest(mergeable=mergeable):
                detail = deepcopy(self.pull_request)
                detail["mergeable"] = mergeable
                api.side_effect = [[self.pull_request], detail]
                self.assertIsNone(self.coverage())

    @patch("ci_changes.github_json")
    def test_pr_changed_or_closed_during_lookup_builds_push(self, api):
        for mutation in ("advanced", "closed"):
            with self.subTest(mutation=mutation):
                detail = deepcopy(self.pull_request)
                if mutation == "advanced":
                    detail["head"]["sha"] = "c" * 40
                else:
                    detail["state"] = "closed"
                api.side_effect = [[self.pull_request], detail]
                self.assertIsNone(self.coverage())

    @patch("ci_changes.github_json")
    def test_api_errors_and_invalid_responses_build_push(self, api):
        for failure in (URLError("unavailable"), TimeoutError(), ValueError("bad JSON")):
            with self.subTest(failure=type(failure).__name__):
                api.side_effect = failure
                self.assertIsNone(self.coverage())
        api.side_effect = None
        for response in ({"message": "unexpected"}, [None, {}, {"head": None}]):
            with self.subTest(response=response):
                api.return_value = response
                self.assertIsNone(self.coverage())

    @patch("ci_changes.github_json")
    def test_other_events_and_branches_never_use_pr_coverage(self, api):
        for event_name in ("pull_request", "workflow_dispatch"):
            self.assertIsNone(self.coverage(event_name))
        main_push = deepcopy(self.event)
        main_push["ref"] = "refs/heads/main"
        self.assertIsNone(self.coverage(event=main_push))
        api.assert_not_called()

    @patch("ci_changes.github_json")
    def test_invalid_push_identity_and_missing_token_never_skip(self, api):
        for sha in ("", "0" * 40, "not-a-sha"):
            event = deepcopy(self.event)
            event["after"] = sha
            self.assertIsNone(self.coverage(event=event))
        event = deepcopy(self.event)
        event["repository"]["full_name"] = "other/repository"
        self.assertIsNone(self.coverage(event=event))
        self.assertIsNone(matching_open_pull_request(
            "push", self.event, self.repository, "",
        ))
        api.assert_not_called()

    @patch("ci_changes.github_json")
    @patch("ci_changes.changes_require_build", return_value=True)
    def test_build_selection_delegates_only_confirmed_push(self, changes, api):
        with patch.dict(os.environ, {
            "GITHUB_REPOSITORY": self.repository, "GITHUB_TOKEN": "test-token",
            "GITHUB_API_URL": "https://api.github.com",
        }):
            api.side_effect = [[self.pull_request], self.pull_request]
            build, reason = select_build("push", self.event)
            self.assertFalse(build)
            self.assertIn("PR #7", reason)
            api.reset_mock()
            build, _ = select_build("pull_request", self.event)
            self.assertTrue(build)
            api.assert_not_called()

    @patch("ci_changes.matching_open_pull_request")
    @patch("ci_changes.changes_require_build", return_value=False)
    def test_documentation_skip_needs_no_api(self, changes, coverage):
        build, reason = select_build("push", self.event)
        self.assertFalse(build)
        self.assertIn("documentation", reason)
        coverage.assert_not_called()


if __name__ == "__main__":
    unittest.main()
