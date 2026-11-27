#!/usr/bin/env python3
"""Check for and prepare upstream merges. The same file is in every FlowApps fork.

    python3 rebrand/upstream.py status [--json]   # is upstream ahead of us?
    python3 rebrand/upstream.py report --out FILE # status, plus a trial merge and rebrand
    python3 rebrand/upstream.py prepare --body FILE [--branch NAME]
                                                  # merge on a new branch, rebrand, commit

`rebrand/upstream.json` says what we follow:

    {"repo": "owner/name", "follows": "branch", "branch": "main", "commit": "<sha>"}
    {"repo": "owner/name", "tag": "v1.2.3"}         # follows upstream's latest release
    "conflicts": "theirs" | "stop"                  # optional, default "stop"

With "theirs", a conflicted file takes upstream's version and `rebrand/apply.py` brands it again;
use it only for thin rebrand forks. With "stop", prepare refuses and lists the conflicts, so a
person merges by hand (docs/UPSTREAM.md). Either way:

- files marked `merge=ours` in .gitattributes keep our version;
- `.github/workflows/` always keeps ours, and the pull request lists what upstream changed there;
- nothing is pushed: the "Upstream prepare" workflow pushes the branch and opens the pull request.

`--github-output` also writes the results to $GITHUB_OUTPUT for the workflows.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "rebrand" / "upstream.json"
WORKFLOWS = ".github/workflows/"
LIST_LIMIT = 40


def git(*args: str, cwd: Path = ROOT, check: bool = True) -> str:
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if check and r.returncode != 0:
        sys.exit(f"git {' '.join(args)} failed:\n{r.stderr.strip()}")
    return r.stdout.strip()


def load() -> dict:
    cfg = json.loads(CONFIG.read_text(encoding="utf-8"))
    if "repo" not in cfg:
        sys.exit("rebrand/upstream.json has no \"repo\"")
    return cfg


def latest_release(repo: str) -> str:
    req = urllib.request.Request(f"https://api.github.com/repos/{repo}/releases/latest")
    req.add_header("Accept", "application/vnd.github+json")
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)["tag_name"]


def fetch(cfg: dict) -> tuple[str, str]:
    """Fetch upstream; return (ref to merge, upstream's name for it)."""
    url = f"https://github.com/{cfg['repo']}.git"
    have = git("remote", "get-url", "upstream", check=False)
    if not have:
        git("remote", "add", "upstream", url)
        git("config", "remote.upstream.tagOpt", "--no-tags")
    elif have.removesuffix(".git").rstrip("/").lower() != url.removesuffix(".git").lower():
        sys.exit(f"remote 'upstream' is {have}, but rebrand/upstream.json says {cfg['repo']}")
    if "tag" in cfg and cfg.get("follows", "tag") == "tag":
        tag = latest_release(cfg["repo"])
        git("fetch", "--quiet", "--no-tags", "upstream", f"+refs/tags/{tag}:refs/tags/upstream/{tag}")
        return f"refs/tags/upstream/{tag}", tag
    branch = cfg.get("branch", "main")
    git("fetch", "--quiet", "--no-tags", "upstream", f"+refs/heads/{branch}:refs/remotes/upstream/{branch}")
    ref = f"refs/remotes/upstream/{branch}"
    return ref, git("rev-parse", "--short=12", ref)


def status(cfg: dict) -> dict:
    ref, theirs = fetch(cfg)
    ours = cfg.get("tag") if "tag" in cfg and cfg.get("follows", "tag") == "tag" else cfg.get("commit", "")[:12]
    count = int(git("rev-list", "--count", f"HEAD..{ref}"))
    commits = git("log", "--no-merges", f"--max-count={LIST_LIMIT}", "--format=%h %s", f"HEAD..{ref}")
    return {
        "repo": cfg["repo"],
        "ours": ours or "(not recorded)",
        "theirs": theirs,
        "ref": ref,
        "new_commits": count,
        "up_to_date": count == 0,
        "commits": commits.splitlines() if commits else [],
        "compare_url": f"https://github.com/{cfg['repo']}/compare/{ours}...{theirs}" if ours else "",
    }


def merge(ref: str, cfg: dict, cwd: Path) -> tuple[list[str], list[str], bool]:
    """Merge without committing. Return (conflicts, upstream workflow changes, resolved?)."""
    r = subprocess.run(
        ["git", "-c", "merge.ours.driver=true", "merge", "--no-commit", "--no-ff", ref],
        cwd=cwd, capture_output=True, text=True,
    )
    conflicts = git("diff", "--name-only", "--diff-filter=U", cwd=cwd).splitlines()
    if r.returncode != 0 and not conflicts:
        sys.exit(f"git merge failed:\n{r.stderr.strip() or r.stdout.strip()}")
    # Upstream's CI is not ours: keep our workflows, and say what upstream changed.
    workflows = git("diff", "--name-only", "HEAD", "--", WORKFLOWS, cwd=cwd).splitlines()
    workflows += [p for p in conflicts if p.startswith(WORKFLOWS) and p not in workflows]
    for p in workflows:
        if subprocess.run(["git", "cat-file", "-e", f"HEAD:{p}"], cwd=cwd, capture_output=True).returncode == 0:
            git("checkout", "HEAD", "--", p, cwd=cwd)
        else:
            git("rm", "-q", "-f", "--", p, cwd=cwd, check=False)
    conflicts = [p for p in conflicts if not p.startswith(WORKFLOWS)]
    if conflicts and cfg.get("conflicts", "stop") == "theirs":
        for p in conflicts:
            if p == "README.md":
                git("checkout", "--ours", "--", p, cwd=cwd)
            elif subprocess.run(["git", "checkout", "--theirs", "--", p], cwd=cwd, capture_output=True).returncode != 0:
                git("rm", "-q", "-f", "--", p, cwd=cwd)
                continue
            git("add", "--", p, cwd=cwd)
        return conflicts, workflows, True
    return conflicts, workflows, not conflicts


def rebrand(cwd: Path) -> tuple[bool, str]:
    script = cwd / "rebrand" / "apply.py"
    out = subprocess.run([sys.executable, str(script)], cwd=cwd, capture_output=True, text=True)
    chk = subprocess.run([sys.executable, str(script), "--check"], cwd=cwd, capture_output=True, text=True)
    text = (out.stdout + out.stderr + chk.stdout + chk.stderr).strip()
    return out.returncode == 0 and chk.returncode == 0, "\n".join(text.splitlines()[-30:])


def describe(st: dict, conflicts: list[str], workflows: list[str], resolved: bool, cfg: dict,
             branded: bool | None, log: str) -> str:
    lines = [f"**{st['repo']}** is {st['new_commits']} commit(s) ahead: we're on `{st['ours']}`, upstream is on `{st['theirs']}`."]
    if st["compare_url"]:
        lines.append(f"\nChanges: {st['compare_url']}")
    if st["commits"]:
        lines.append("\n<details><summary>Upstream commits</summary>\n")
        lines += [f"- {c}" for c in st["commits"]]
        if st["new_commits"] > len(st["commits"]):
            lines.append(f"- … and {st['new_commits'] - len(st['commits'])} more")
        lines.append("\n</details>")
    lines.append("\n## Merge")
    if not conflicts:
        lines.append("Merges cleanly.")
    elif resolved:
        lines.append("These conflicted and took upstream's version, then the rebrand ran again (`\"conflicts\": \"theirs\"`):")
        lines += [f"- `{p}`" for p in conflicts]
    else:
        lines.append("These conflict and need a person (docs/UPSTREAM.md, \"Merging upstream\"):")
        lines += [f"- `{p}`" for p in conflicts]
    if workflows:
        lines.append("\nUpstream changed these workflows; ours were kept. Bring any change in by hand if we want it:")
        lines += [f"- `{p}`" for p in workflows]
    if branded is not None:
        lines.append("\n## Rebrand")
        lines.append("`apply.py --check` passes." if branded else
                     "**The rebrand needs attention**: upstream changed something `apply.py` edits. See its output below and docs/UPSTREAM.md.")
        if log:
            lines += ["", "```", log, "```"]
    return "\n".join(lines) + "\n"


def to_github(**values: object) -> None:
    path = os.environ.get("GITHUB_OUTPUT")
    if not path:
        return
    with open(path, "a", encoding="utf-8") as f:
        for k, v in values.items():
            f.write(f"{k}={v}\n")


def cmd_status(args: argparse.Namespace) -> None:
    st = status(load())
    if args.json:
        print(json.dumps(st, indent=2))
    else:
        print("Up to date with upstream." if st["up_to_date"] else
              f"{st['repo']}: {st['new_commits']} new commit(s), ours {st['ours']}, theirs {st['theirs']}")
    if args.github_output:
        to_github(new_commits=st["new_commits"], theirs=st["theirs"])


def cmd_report(args: argparse.Namespace) -> None:
    cfg = load()
    st = status(cfg)
    if st["up_to_date"]:
        text = f"Up to date with {st['repo']} (`{st['theirs']}`).\n"
    else:
        tmp = Path(tempfile.mkdtemp(prefix="upstream-trial-"))
        try:
            git("worktree", "add", "--quiet", "--detach", str(tmp), "HEAD")
            conflicts, workflows, resolved = merge(st["ref"], cfg, tmp)
            branded, log = rebrand(tmp) if resolved else (None, "")
            text = describe(st, conflicts, workflows, resolved, cfg, branded, log)
            text += "\nTo bring these in, run the **Upstream prepare** workflow (Actions tab): it merges on a new branch and opens a pull request for review.\n"
        finally:
            git("worktree", "remove", "--force", str(tmp), check=False)
            shutil.rmtree(tmp, ignore_errors=True)
    Path(args.out).write_text(text, encoding="utf-8")
    print(text)
    if args.github_output:
        to_github(new_commits=st["new_commits"], theirs=st["theirs"])


def cmd_prepare(args: argparse.Namespace) -> None:
    cfg = load()
    if git("status", "--porcelain", "--untracked-files=no"):
        sys.exit("The working tree has changes; commit or stash them first.")
    st = status(cfg)
    if st["up_to_date"]:
        print(f"Up to date with {st['repo']}; nothing to prepare.")
        to_github(branch="")
        return
    branch = args.branch or f"upstream-sync/{datetime.date.today().isoformat()}"
    start = git("rev-parse", "--abbrev-ref", "HEAD")
    git("checkout", "-q", "-b", branch)
    conflicts, workflows, resolved = merge(st["ref"], cfg, ROOT)
    if not resolved:
        print(describe(st, conflicts, workflows, resolved, cfg, None, ""))
        git("merge", "--abort", check=False)
        git("checkout", "-q", start)
        git("branch", "-q", "-D", branch)
        sys.exit("Stopped: conflicts need a person (docs/UPSTREAM.md). Nothing was changed.")
    branded, log = rebrand(ROOT)
    if not branded:
        print(log)
        sys.exit(f"Stopped: the rebrand needs attention. The merge is left uncommitted on {branch}.")
    if "tag" in cfg and cfg.get("follows", "tag") == "tag":
        cfg["tag"] = st["theirs"]
    else:
        cfg["commit"] = git("rev-parse", st["ref"])
        cfg["date"] = datetime.date.today().isoformat()
    CONFIG.write_text(json.dumps(cfg, indent=2) + "\n", encoding="utf-8")
    git("add", "-A")
    git("commit", "-q", "-m",
        f"chore: merge upstream {st['repo']} {st['theirs']}\n\n"
        f"{st['new_commits']} upstream commit(s). Prepared by rebrand/upstream.py.")
    body = describe(st, conflicts, workflows, resolved, cfg, branded, "")
    body += ("\n## Before merging\n- [ ] CI passes (failures that also fail on main don't count)\n"
             "- [ ] Run the app and check the name, icons and About\n"
             "- [ ] Merge with a **merge commit**, never squash, so the next merge finds its base\n")
    Path(args.body).write_text(body, encoding="utf-8")
    print(f"Prepared {branch}: {st['new_commits']} upstream commit(s), {len(conflicts)} resolved conflict(s).")
    to_github(branch=branch, theirs=st["theirs"], new_commits=st["new_commits"],
              title=f"Merge upstream {st['repo']} {st['theirs']}")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("status")
    s.add_argument("--json", action="store_true")
    s.add_argument("--github-output", action="store_true")
    s.set_defaults(func=cmd_status)
    r = sub.add_parser("report")
    r.add_argument("--out", required=True)
    r.add_argument("--github-output", action="store_true")
    r.set_defaults(func=cmd_report)
    pr = sub.add_parser("prepare")
    pr.add_argument("--body", required=True)
    pr.add_argument("--branch")
    pr.add_argument("--github-output", action="store_true")
    pr.set_defaults(func=cmd_prepare)
    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
