#!/usr/bin/env python3
"""Publishes a release of the model railroading addon, so Leah installs it by the repository's address:

    leah --install-addon https://github.com/bmasters-dev/model-railroading
    leah --update-addon model_railroading

    python3 -I leah-addon/tools/publish_release.py

Builds the addon (release profile) here, names its library as Leah looks for it
(`model_railroading-linux-x86_64.so`, with its `.sha256`), tags the pushed commit
`leah-addon-v<version>` (the version in leah-addon/Cargo.toml; the repository holds other things too,
so its addon's releases are tagged apart: Leah finds the newest release holding the addon's file,
bmasters-dev/leah-language#719), and publishes a GitHub release of that tag with the two files and
the icon pack. Built here rather than on GitHub's machines until Leah's addon kit is published
(bmasters-dev/leah-language#720). Signed in with the token in ~/Leah_Secrets/github_token.txt, read
when it is needed.
"""
import hashlib
import json
import platform
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ADDON = "model_railroading"
REPO = "bmasters-dev/model-railroading"
HERE = Path(__file__).resolve().parent.parent
TOP = HERE.parent
DIST = TOP / "dist"


def run(*words, at=TOP):
    done = subprocess.run(words, cwd=at, capture_output=True, text=True)
    if done.returncode != 0:
        raise SystemExit(f"publish_release: {' '.join(words)} failed:\n{done.stdout[-1500:]}{done.stderr[-1500:]}")
    return done.stdout.strip()


def token():
    return (Path.home() / "Leah_Secrets" / "github_token.txt").read_text().strip()


def github(method, url, data=None, body=None, kind="application/json"):
    if data is not None:
        body = json.dumps(data).encode()
    request = urllib.request.Request(url, data=body, method=method, headers={"Authorization": f"Bearer {token()}", "Accept": "application/vnd.github+json", "Content-Type": kind, "User-Agent": "model-railroading-release"})
    try:
        with urllib.request.urlopen(request) as answer:
            return json.loads(answer.read() or b"null")
    except urllib.error.HTTPError as e:
        raise SystemExit(f"publish_release: GitHub answered {e.code} to {method} {url}: {e.read()[:500].decode(errors='replace')}")


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__.split("\n\n")[1])
    version = re.search(r'^version = "(.*)"$', (HERE / "Cargo.toml").read_text(), re.M).group(1)
    told = re.search(r'^version = "(.*)"$', (HERE / "addon.toml").read_text(), re.M).group(1)
    if told != version:
        raise SystemExit(f"publish_release: addon.toml says {told}, Cargo.toml {version}: make them the same")
    tag = f"leah-addon-v{version}"
    # (Only what is committed and pushed is published.)
    if run("git", "status", "--porcelain"):
        raise SystemExit("publish_release: commit everything first")
    run("git", "fetch", "--quiet", "origin")
    if run("git", "rev-parse", "HEAD") != run("git", "rev-parse", "origin/main"):
        raise SystemExit("publish_release: push main first (the release is of the commit GitHub has)")
    if run("git", "tag", "--list", tag):
        raise SystemExit(f"publish_release: {tag} exists already: raise the version in leah-addon/Cargo.toml and addon.toml")
    run("cargo", "build", "--release", at=HERE)
    arch = platform.machine()
    file = f"{ADDON}-linux-{arch}.so"
    shutil.rmtree(DIST, ignore_errors=True)
    DIST.mkdir()
    shutil.copy(HERE / "target" / "release" / f"libleah_addon_{ADDON}.so", DIST / file)
    run("strip", "--strip-unneeded", str(DIST / file))
    digest = hashlib.sha256((DIST / file).read_bytes()).hexdigest()
    (DIST / f"{file}.sha256").write_text(f"{digest}  {file}\n")
    shutil.copy(HERE / f"{ADDON}.icons", DIST / f"{ADDON}.icons")
    # (Its help for Leah's API and the IDE's hover help, from the pages: leah-language#727.)
    run(sys.executable, "-I", str(HERE / "tools" / "docs.py"), "help", "--to", str(DIST / f"{ADDON}.help.json"), at=HERE)
    run("git", "tag", tag)
    run("git", "push", "--quiet", "origin", tag)
    release = github("POST", f"https://api.github.com/repos/{REPO}/releases", {
        "tag_name": tag,
        "name": f"Model railroading addon {version}",
        "body": f"The model railroading addon for Leah, {version}.\n\nInstall it with:\n\n    leah --install-addon https://github.com/{REPO}\n\nand update it with `leah --update-addon {ADDON}`.\n",
    })
    for name in [file, f"{file}.sha256", f"{ADDON}.icons", f"{ADDON}.help.json"]:
        print(f"uploading {name}")
        github("POST", f"https://uploads.github.com/repos/{REPO}/releases/{release['id']}/assets?name={name}", body=(DIST / name).read_bytes(), kind="application/octet-stream")
    print(f"published {release['html_url']}")


if __name__ == "__main__":
    main()
