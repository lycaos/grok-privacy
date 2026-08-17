"""Tests for `patchctl apply --auto-resolve-conflicts`.

Each test builds a throwaway git repo: a locked upstream base, a one-patch
privacy queue whose patch conflicts with the new upstream commit, and a stub
conflict resolver injected via PATCHCTL_CONFLICT_RESOLVER_CMD. The fixture
contract greps for the patch's guard, so a resolution that drops the privacy
guarantee must fail the contract gate, not just the merge.

Run directly: python3 maint/scripts/tests/test_patchctl_auto_resolve.py
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]  # maint/scripts
PATCHCTL = SCRIPTS / "patchctl.py"

# `fn other` is identical everywhere: it is never part of the conflict, so a
# resolution has no business touching it.
BASE_APP = """fn accept() {
    record_local();
    record_upstream();
}

fn other() {
    keep_me();
}
"""

PATCHED_APP = """fn accept() {
    record_local();
    if !privacy() {
        record_upstream();
    }
}

fn other() {
    keep_me();
}
"""

UPSTREAM_APP = """fn accept() {
    if has_account() {
        record_local();
    }
    record_upstream();
}

fn other() {
    keep_me();
}
"""

MERGED_APP = """fn accept() {
    if has_account() {
        record_local();
    }
    if !privacy() {
        record_upstream();
    }
}

fn other() {
    keep_me();
}
"""

# Correct conflict resolution, but `fn other` vanished: a truncated resolver
# output must be rejected even though the privacy contract would still pass.
TRUNCATED_APP = """fn accept() {
    if has_account() {
        record_local();
    }
    if !privacy() {
        record_upstream();
    }
}
"""

# Committable (differs from upstream) but drops the privacy guard: only the
# contract gate can catch this one.
DROPS_GUARD_APP = """fn accept() {
    if has_account() {
        record_local();
    }
    // resolved
    record_upstream();
}

fn other() {
    keep_me();
}
"""

VERSION_CARGO = '[package]\nname = "xai-grok-version"\nversion = "0.1.0"\n'

CONTRACTS_TOML = """schema = 1

[[contract]]
id = "guard-present"
group = "privacy"
min_tests = 1
command = [
  "sh", "-c",
  "grep -q 'if !privacy()' src/app.rs && echo 'test result: ok. 1 passed'",
]
"""


def patchset_toml(contracts: tuple[str, ...]) -> str:
    ids = ", ".join(f'"{c}"' for c in contracts)
    return (
        "schema = 1\n\n"
        "[[patch]]\n"
        'id = "guard-upstream"\n'
        'file = "0001-guard-upstream.patch"\n'
        "critical = true\n"
        f"contracts = [{ids}]\n"
    )


class Fixture:
    def __init__(self, tmp: Path, *, contracts: tuple[str, ...] = ("guard-present",)):
        self.tmp = tmp
        self.root = tmp / "repo"
        self.root.mkdir()
        self.env = dict(
            os.environ,
            GIT_AUTHOR_NAME="Test",
            GIT_AUTHOR_EMAIL="test@example.invalid",
            GIT_COMMITTER_NAME="Test",
            GIT_COMMITTER_EMAIL="test@example.invalid",
        )
        self._build(contracts)

    def sh(self, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        proc = subprocess.run(
            list(args), cwd=self.root, env=self.env, text=True, capture_output=True
        )
        if check and proc.returncode != 0:
            raise AssertionError(
                f"fixture command failed: {args}\n{proc.stdout}\n{proc.stderr}"
            )
        return proc

    def git(self, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        return self.sh("git", *args, check=check)

    def _build(self, contracts: tuple[str, ...]) -> None:
        self.git("init", "-b", "main")
        # Upstream base.
        cargo = self.root / "crates/codegen/xai-grok-version/Cargo.toml"
        cargo.parent.mkdir(parents=True)
        cargo.write_text(VERSION_CARGO, encoding="utf-8")
        app = self.root / "src/app.rs"
        app.parent.mkdir(parents=True)
        app.write_text(BASE_APP, encoding="utf-8")
        self.git("add", "-A")
        self.git("commit", "-m", "base")
        self.base_sha = self.git("rev-parse", "HEAD").stdout.strip()

        # The privacy patch, exported then reset away.
        app.write_text(PATCHED_APP, encoding="utf-8")
        self.git("add", "-A")
        self.git(
            "commit",
            "-m",
            "guard-upstream: keep acks local\n\nGork-Patch-Id: guard-upstream\n",
        )
        patch_out = self.tmp / "patch-out"
        self.git("format-patch", "-1", "HEAD", "-o", str(patch_out))
        exported = sorted(patch_out.glob("*.patch"))
        assert len(exported) == 1, exported
        self.git("reset", "--hard", self.base_sha)

        # The new upstream commit rewrites the same lines: guaranteed conflict.
        self.git("switch", "-c", "upstream-main")
        app.write_text(UPSTREAM_APP, encoding="utf-8")
        self.git("add", "-A")
        self.git("commit", "-m", "upstream rework")
        self.up_sha = self.git("rev-parse", "HEAD").stdout.strip()

        # Control plane on the work branch (where apply is launched from).
        self.git("switch", "-c", "work", "main")
        maint = self.root / "maint"
        patches = maint / "patches"
        patches.mkdir(parents=True)
        patch_file = patches / "0001-guard-upstream.patch"
        shutil.copy2(exported[0], patch_file)
        digest = hashlib.sha256(patch_file.read_bytes()).hexdigest()
        (patches / "SHA256SUMS").write_text(
            f"{digest}  0001-guard-upstream.patch\n", encoding="utf-8"
        )
        (patches / "series").write_text(
            "# test series\n0001-guard-upstream.patch\n", encoding="utf-8"
        )
        (maint / "patchset.toml").write_text(
            patchset_toml(contracts), encoding="utf-8"
        )
        (maint / "control-files.toml").write_text(
            'paths = ["maint"]\ntemplate_root = "maint/control"\n', encoding="utf-8"
        )
        (maint / "upstream.lock.toml").write_text(
            "schema = 1\n"
            'repository = "local"\n'
            f'commit = "{self.base_sha}"\n'
            'source_rev = ""\n'
            'version = "0.1.0"\n'
            "patchset_revision = 1\n"
            'patch_tip = ""\n',
            encoding="utf-8",
        )
        contracts_dir = maint / "contracts"
        contracts_dir.mkdir()
        (contracts_dir / "privacy-contract.toml").write_text(
            CONTRACTS_TOML, encoding="utf-8"
        )
        scripts_dir = maint / "scripts"
        scripts_dir.mkdir()
        shutil.copy2(SCRIPTS / "verify_privacy_contract.py", scripts_dir)
        self.git("add", "-A")
        self.git("commit", "-m", "control plane")

    def resolver(self, name: str, body: str) -> str:
        path = self.tmp / name
        path.write_text(body, encoding="utf-8")
        path.chmod(0o755)
        return str(path)

    def resolver_printing(self, name: str, content: str) -> str:
        quoted = content.replace("'", "'\\''")
        return self.resolver(
            name, f"#!/bin/sh\ncat > /dev/null\nprintf '%s' '{quoted}'\n"
        )

    def apply(
        self, resolver_cmd: str, *extra: str
    ) -> subprocess.CompletedProcess[str]:
        env = dict(self.env, PATCHCTL_CONFLICT_RESOLVER_CMD=resolver_cmd)
        return subprocess.run(
            [
                sys.executable,
                str(PATCHCTL),
                "apply",
                "--upstream",
                self.up_sha,
                "--auto-resolve-conflicts",
                *extra,
            ],
            cwd=self.root,
            env=env,
            text=True,
            capture_output=True,
        )

    def app_content(self) -> str:
        return (self.root / "src/app.rs").read_text(encoding="utf-8")

    def am_in_progress(self) -> bool:
        return (self.root / ".git/rebase-apply").exists()

    def status_json(self) -> dict:
        return json.loads(
            (self.root / "maint/last-apply-status.json").read_text(encoding="utf-8")
        )


class AutoResolveTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="patchctl-autoresolve-")
        self.addCleanup(self._tmp.cleanup)
        self.tmp = Path(self._tmp.name)

    def test_resolves_conflict_and_passes_contracts(self) -> None:
        fx = Fixture(self.tmp)
        good = fx.resolver_printing("resolver-good.sh", MERGED_APP)
        proc = fx.apply(good)
        self.assertEqual(
            proc.returncode, 0, f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
        self.assertEqual(fx.app_content(), MERGED_APP)
        subjects = fx.git("log", "--format=%s").stdout
        self.assertIn("guard-upstream: keep acks local", subjects)
        self.assertIn("AUTO-RESOLVED", proc.stdout)
        status = fx.status_json()
        self.assertEqual(len(status["auto_resolved"]), 1)
        self.assertEqual(
            status["auto_resolved"][0]["patch"], "0001-guard-upstream.patch"
        )
        self.assertEqual(status["auto_resolved"][0]["method"], "ai")

    def test_rejects_output_with_conflict_markers(self) -> None:
        fx = Fixture(self.tmp)
        bad = fx.resolver_printing(
            "resolver-markers.sh",
            "<<<<<<< ours\n" + UPSTREAM_APP + ">>>>>>> theirs\n",
        )
        proc = fx.apply(bad)
        self.assertEqual(proc.returncode, 3, proc.stderr)
        self.assertFalse(fx.am_in_progress())
        report = self.tmp / "repo/.git/grok-apply-conflict.diff"
        self.assertTrue(report.is_file())
        self.assertIn(
            "rejected resolver proposal", report.read_text(encoding="utf-8")
        )

    def test_resolver_failure_falls_back_to_fail_closed(self) -> None:
        fx = Fixture(self.tmp)
        failing = fx.resolver("resolver-fails.sh", "#!/bin/sh\nexit 7\n")
        proc = fx.apply(failing)
        self.assertEqual(proc.returncode, 3, proc.stderr)
        self.assertFalse(fx.am_in_progress())
        self.assertIn("auto-resolve failed", proc.stderr)

    def test_contract_failure_fails_closed(self) -> None:
        # Resolver silently drops the privacy guard: the merge succeeds but the
        # contract gate must catch it.
        fx = Fixture(self.tmp)
        dropper = fx.resolver_printing("resolver-drops-guard.sh", DROPS_GUARD_APP)
        proc = fx.apply(dropper)
        self.assertEqual(proc.returncode, 3, proc.stdout + proc.stderr)
        self.assertIn("contract", (proc.stdout + proc.stderr).lower())
        self.assertFalse(fx.am_in_progress())

    def test_no_contracts_disables_ai_resolution(self) -> None:
        fx = Fixture(self.tmp, contracts=())
        good = fx.resolver_printing("resolver-good.sh", MERGED_APP)
        proc = fx.apply(good)
        self.assertEqual(proc.returncode, 3, proc.stdout + proc.stderr)
        self.assertIn("no contracts", proc.stdout + proc.stderr)
        self.assertFalse(fx.am_in_progress())

    def test_rejects_edits_outside_conflict_regions(self) -> None:
        # The resolved conflict is correct, but the resolver also dropped
        # `fn other` — content it had no conflict to resolve. The privacy
        # contract alone cannot see that; the common-segment check must.
        fx = Fixture(self.tmp)
        truncating = fx.resolver_printing("resolver-truncates.sh", TRUNCATED_APP)
        proc = fx.apply(truncating)
        self.assertEqual(proc.returncode, 3, proc.stdout + proc.stderr)
        self.assertIn("outside the conflict regions", proc.stderr)
        self.assertFalse(fx.am_in_progress())

    def test_rerere_replays_recorded_resolution(self) -> None:
        fx = Fixture(self.tmp)
        good = fx.resolver_printing("resolver-good.sh", MERGED_APP)
        first = fx.apply(good)
        self.assertEqual(first.returncode, 0, first.stderr)
        # Second run from scratch: the resolver is broken on purpose — only the
        # recorded resolution can succeed.
        failing = fx.resolver("resolver-fails.sh", "#!/bin/sh\nexit 7\n")
        second = fx.apply(failing, "--force")
        self.assertEqual(
            second.returncode, 0, f"stdout:\n{second.stdout}\nstderr:\n{second.stderr}"
        )
        self.assertEqual(fx.app_content(), MERGED_APP)
        self.assertEqual(fx.status_json()["auto_resolved"][0]["method"], "rerere")


if __name__ == "__main__":
    unittest.main(verbosity=2)
