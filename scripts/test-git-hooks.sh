#!/usr/bin/env bash
set -euo pipefail

# Use disposable repositories: never switch the real checkout to main or push it.
repository_root=$(cd "$(dirname "$0")/.." && pwd)
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT
git init --quiet --bare "$test_root/remote.git"
git init --quiet --initial-branch=main "$test_root/work"
cd "$test_root/work"
git config user.name 'Hook test'
git config user.email 'hook-test@example.invalid'
git config commit.gpgsign false
git config core.hooksPath "$repository_root/.githooks"
git remote add origin "$test_root/remote.git"

expect_rejected() {
    local expected=$1
    shift
    if "$@" > "$test_root/output" 2>&1; then
        echo "Expected rejection: $*" >&2
        exit 1
    fi
    if ! grep -Fq "$expected" "$test_root/output"; then
        cat "$test_root/output" >&2
        exit 1
    fi
}

expect_rejected 'Direct commits to main are blocked' git commit --quiet --allow-empty -m 'test: blocked main commit'
git switch --quiet -c codex/dev
git commit --quiet --allow-empty -m 'test: development branch is allowed'
git push --quiet origin HEAD:refs/heads/codex/dev
expect_rejected 'Direct pushes to refs/heads/main are blocked' git push origin HEAD:refs/heads/main
expect_rejected 'Direct pushes to refs/heads/master are blocked' git push origin HEAD:refs/heads/master
if git --git-dir="$test_root/remote.git" show-ref --verify --quiet refs/heads/main; then
    echo 'Protected main ref unexpectedly exists on test remote' >&2
    exit 1
fi
git switch --quiet -c master
expect_rejected 'Direct commits to master are blocked' git commit --quiet --allow-empty -m 'test: blocked master commit'
git switch --quiet --detach HEAD
expect_rejected 'detached HEAD' git commit --quiet --allow-empty -m 'test: blocked detached commit'
printf '%s\n' 'PASS: codex/dev commit/push; main/master commit and destination push guards; detached HEAD guard.'
