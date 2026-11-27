# Keeping up with upstream

Mailflow is a fork of [MailCraft](https://github.com/UCS-Care-Group/mailcraft) (MIT or Apache-2.0).
This is how we take their updates without losing our name, icon, links or our own features.

## Two kinds of change

Mailflow changes upstream's files in two different ways, and a merge treats them differently.

1. **Branding** (names, bundle id, settings and cache folders, credential-store name, installer
   names, GUIDs and links, icons, the `NOTICE` paragraph) is made **only by `rebrand/apply.py`**,
   from `rebrand/brand.json`. Never hand-edit these: add an entry to `brand.json` and run the
   script. `apply.py` is the same in every FlowApps fork; only `brand.json` differs.
2. **Features** (writing help, onboarding, colour themes and the theme picker, the ribbon, compose
   and text-menu changes, `ui.aiAssist` / `ui.aiDraft` / `ui.onboarding` / `ui.colorTheme`, the
   Mailflow mark in `crates/ui-egui/src/icons/mark.rs`, and the brand switch in
   `crates/ui-egui/src/i18n.rs`) are ordinary commits. Git carries them through a merge; a
   conflict in one of them is resolved by hand, keeping both sides' intent.

So **do not blanket "take theirs"** on a conflicted file: that is only safe for files whose sole
Mailflow change is branding.

## What lives where

| Ours (upstream has no such file, or we replace theirs whole) | Upstream's, branded only by `rebrand/apply.py` |
|---|---|
| `rebrand/` (`apply.py`, `brand.json`, `app-icon/`) | `apps/mailcraft/build.rs`, `alerts.rs`, `menubar.rs`, `opened_files.rs`, `secrets.rs`, `wipe.rs` |
| `brand/` (gitignored; kept outside the repo) | `crates/control-key/src/lib.rs`, `tests.rs`, `crates/ui-egui/src/account_setup.rs` |
| `docs/UPSTREAM.md`, `README.md`, `README.mailcraft.md`, `CLAUDE.md` | `packaging/macos/*`, `packaging/windows/*` |
| `.github/workflows/mailflow-windows.yml` | `assets/app-icon/` (file names theirs, artwork ours) |
| | `NOTICE`, `.gitignore`, `.gitattributes` |

Files with **both** branding and features (resolve by hand, then re-run `apply.py`):
`apps/mailcraft/src/main.rs`, `crates/ui-egui/src/dialogs.rs`, and any file under the feature
list above.

Left alone on purpose: crate and binary names (`mailcraft*`), environment variables, log-file
names, the tray tooltip, HTTP user agent and MCP server name, licences and credits, and upstream's
docs. Mailflow's settings folder is `Mailflow` and the Windows installer has its own UpgradeCode
and GUIDs, so Mailflow installs beside a MailCraft instead of replacing it.

## Merging upstream

Set the clone up once, so upstream's tags arrive as `upstream/v…` and never collide with ours:

```bash
git remote add upstream https://github.com/UCS-Care-Group/mailcraft.git
git config remote.upstream.tagOpt --no-tags
git config --add remote.upstream.fetch '+refs/tags/*:refs/tags/upstream/*'
git config merge.ours.driver true
```

Never push the `upstream/*` tags. Then:

```bash
git fetch upstream
git checkout -b upstream-sync
git merge upstream/main
```

If there are conflicts:

1. In a branding-only file (right-hand column), take theirs: `git checkout --theirs -- <file>`.
2. In `README.md`, `CLAUDE.md` or our workflow, keep ours: `git checkout --ours -- <file>`.
3. In a file with features, resolve by hand: keep upstream's change and ours.

Then re-apply the branding and check it:

```bash
python3 rebrand/apply.py
python3 rebrand/apply.py --check     # prints nothing and exits 0 when everything is rebranded
```

If the script stops with `could not find … to edit`, upstream changed that line: find where the
text went and update that entry in `rebrand/brand.json`. An edit whose replacement contains its
own search text would apply twice; anchor it on the text around it instead.

Build and test (`cargo xtask ci`), run the app once and look at it (tests don't see icons or the
title), commit and open a pull request.

`rebrand/apply.py` was checked on 2026-11-26 against upstream `64b35c2b9` (the merge-base):
applied to a clean upstream copy, it reproduces every branded file exactly.

## Automatic check and prepare

`rebrand/upstream.py` (the same file in every FlowApps fork) and two workflows do the routine part:

- **Upstream check** runs every Monday and on demand. It keeps one `upstream-update` issue current:
  the new upstream commits, a trial merge, and whether the rebrand still applies. It closes the issue
  once we're up to date.
- **Upstream prepare** runs only by hand. It merges upstream on an `upstream-sync/<date>` branch,
  re-applies the branding, checks it, updates `rebrand/upstream.json` and opens a pull request for
  review. It needs the `UPSTREAM_PUSH_TOKEN` secret (see the workflow). If a conflict or the rebrand
  needs a person, it stops without pushing anything; then merge by hand as above.

`"conflicts"` in `rebrand/upstream.json` is `"stop"`: any conflict stops the prepare step, because this fork carries its own code. Upstream's own workflow changes are
never brought in automatically; the pull request lists them. Locally: `python3 rebrand/upstream.py
status`, `report --out FILE` or `prepare --body FILE`.
