#!/usr/bin/env python3
"""Check the build skip policy, including uncertain history and file moves."""

import subprocess
import unittest
from unittest.mock import patch

from ci_changes import changes_require_build, documentation_only


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


if __name__ == "__main__":
    unittest.main()
