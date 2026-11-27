#!/usr/bin/env python3
"""Turn an upstream checkout into the FlowApps app, using rebrand/brand.json.

Every change this app makes to a file that upstream owns is listed in rebrand/brand.json and
made here, never by hand, so that after merging upstream the whole rebrand can be applied again:

    python3 rebrand/apply.py            # apply (safe to run any number of times)
    python3 rebrand/apply.py --check    # exit 1 if anything is still un-rebranded

brand.json holds:
  * "edits": literal find-and-replace in one file ("regex": true for a regular expression).
    Every occurrence is replaced. An edit whose text and replacement are both missing means
    upstream changed that line, and the script stops naming it instead of skipping it;
  * "prepend" / "append": text added once to a file, recognised by its "marker";
  * patches in rebrand/patches/*.patch for code changes too big for find and replace;
  * our icons, copied from rebrand/app-icon/ over assets/app-icon/.

This file is the same in every FlowApps fork; only brand.json differs. See docs/UPSTREAM.md.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HERE = ROOT / "rebrand"
BRAND = json.loads((HERE / "brand.json").read_text(encoding="utf-8"))

CHECK = "--check" in sys.argv
changed: list[str] = []
problems: list[str] = []


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, old: str, new: str) -> None:
    if old == new:
        return
    rel = str(path.relative_to(ROOT))
    if rel not in changed:
        changed.append(rel)
    if not CHECK:
        path.write_text(new, encoding="utf-8", newline="")


def git(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)


def patches() -> None:
    for patch in sorted((HERE / "patches").glob("*.patch")):
        rel = str(patch.relative_to(ROOT))
        if git("apply", "--reverse", "--check", rel).returncode == 0:
            continue  # already applied
        changed.append(f"{rel} (not applied)")
        if CHECK:
            continue
        if git("apply", rel).returncode == 0:
            continue
        r = git("apply", "--3way", rel)
        if r.returncode != 0:
            problems.append(f"{rel}: does not apply cleanly; fix the conflict markers it left, then refresh the patch (docs/UPSTREAM.md)\n{r.stderr.strip()}")


def edits() -> None:
    for e in BRAND.get("edits", []):
        rel, find, repl = e["file"], e["find"], e["replace"]
        path = ROOT / rel
        if not path.exists():
            problems.append(f"{rel}: file is gone (upstream moved or removed it)")
            continue
        old = read(path)
        if e.get("regex"):
            new, n = re.subn(find, repl, old, flags=re.M)
            done = e["done"]
        else:
            n = old.count(find)
            new = old.replace(find, repl)
            done = e.get("done", repl)
        if n == 0 and done not in old:
            problems.append(f"{rel}: could not find {find!r} to edit, and {done!r} is not there either")
            continue
        write(path, old, new)


def additions() -> None:
    for where in ("prepend", "append"):
        for a in BRAND.get(where, []):
            path = ROOT / a["file"]
            old = read(path) if path.exists() else ""
            if a["marker"] in old:
                continue
            text = "\n".join(a["lines"]) + "\n"
            new = text + old if where == "prepend" else old.rstrip("\n") + "\n" + text
            write(path, old, new)


def icons() -> None:
    src = HERE / "app-icon"
    for f in sorted(p for p in src.rglob("*") if p.is_file()):
        dest = ROOT / "assets/app-icon" / f.relative_to(src)
        if dest.exists() and dest.read_bytes() == f.read_bytes():
            continue
        changed.append(str(dest.relative_to(ROOT)))
        if not CHECK:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(f, dest)


def rustfmt() -> None:
    if CHECK or not any(c.endswith(".rs") or "patch" in c for c in changed) or not shutil.which("cargo"):
        return
    subprocess.run(["cargo", "fmt", "--all"], cwd=ROOT, check=False)


def main() -> int:
    patches()
    edits()
    additions()
    icons()
    rustfmt()

    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if CHECK:
        for c in changed:
            print(f"not rebranded: {c}")
        return 1 if (changed or problems) else 0
    print(f"{len(changed)} file(s) changed" if changed else "already rebranded: nothing to change")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
