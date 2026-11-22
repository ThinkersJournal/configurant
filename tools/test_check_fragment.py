#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Tests for check-fragment.py: run `python tools/test_check_fragment.py`. Each case builds a throwaway git repo."""
import os
import shutil
import subprocess  # nosec B404 - runs `git` and the script under test in a temp repo
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-fragment.py")
GIT = shutil.which("git")
MANIFEST = '[package]\nname = "{n}"\nversion = "{v}"\n'
FRAGMENT = "kind: x\n---\nbody\n"


def run(cwd, *args):
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True)  # nosec B603 - fixed arguments


def write(root, path, text):
    full = os.path.join(root, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8", newline="") as f:
        f.write(text)


class CheckFragment(unittest.TestCase):
    def repo(self):
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d, True)
        for cmd in (["init", "-q", "-b", "main"], ["config", "user.email", "t@t"], ["config", "user.name", "t"],
                    ["config", "commit.gpgsign", "false"]):
            run(d, GIT, *cmd)
        write(d, "Cargo.toml", MANIFEST.format(n="a", v="0.1.0"))
        write(d, "changelog.d/README.md", "x\n")
        run(d, GIT, "add", "-A")
        run(d, GIT, "commit", "-q", "-m", "base")
        run(d, GIT, "checkout", "-q", "-b", "work")
        return d

    def check(self, d):
        run(d, GIT, "add", "-A")
        run(d, GIT, "commit", "-q", "-m", "work")
        r = run(d, sys.executable, SCRIPT, "main")
        return r.returncode, r.stdout

    def test_new_crate_with_a_fragment_passes(self):
        d = self.repo()
        write(d, "crates/b/Cargo.toml", MANIFEST.format(n="b", v="0.1.0"))
        write(d, "changelog.d/b.md", FRAGMENT)
        self.assertEqual(self.check(d)[0], 0)

    def test_bump_of_an_existing_crate_with_a_fragment_still_fails(self):
        d = self.repo()
        write(d, "Cargo.toml", MANIFEST.format(n="a", v="0.1.1"))
        write(d, "changelog.d/b.md", FRAGMENT)
        code, out = self.check(d)
        self.assertEqual(code, 1)
        self.assertIn("both bumps a version", out)

    def test_new_crate_plus_a_real_bump_still_fails(self):
        d = self.repo()
        write(d, "crates/b/Cargo.toml", MANIFEST.format(n="b", v="0.1.0"))
        write(d, "Cargo.toml", MANIFEST.format(n="a", v="0.2.0"))
        write(d, "changelog.d/b.md", FRAGMENT)
        self.assertEqual(self.check(d)[0], 1)

    def test_new_crate_without_a_fragment_fails(self):
        d = self.repo()
        write(d, "crates/b/Cargo.toml", MANIFEST.format(n="b", v="0.1.0"))
        code, out = self.check(d)
        self.assertEqual(code, 1)
        self.assertIn("no CHANGELOG fragment", out)


if __name__ == "__main__":
    unittest.main()
