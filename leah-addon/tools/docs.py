#!/usr/bin/env python3
"""The model_railroading addon's documentation: its pages in docs/ (one Markdown file per page),
checked here and published to this repository's own wiki
(https://github.com/bmasters-dev/model-railroading/wiki), apart from Leah's reference: a 3rd-party
addon keeps its own documentation.

    python3 -I tools/docs.py check [pages...]   run every page's code samples and check its links
    python3 -I tools/docs.py wiki [--no-push]   publish every page to the wiki, with its Home page,
                                                header, and sidebar (only committed pages; never over
                                                an edit made on the website, unless --force)
    python3 -I tools/docs.py help [--to path]   write model_railroading.help.json from the pages: the
                                                help Leah gives for the addon's names (the API's help,
                                                the IDE's hover help), beside the .so (by default in
                                                target/; the release and tools/bench.sh carry it)

Run from leah-addon/ (or give its path in ADDON_ROOT).

Pages
-----
docs/<title>.md is the wiki page <title> (examples.md, the guide's examples, isn't published). A page
starts with "# <title>". Links to other pages are written [text](ref:Title) or [text](ref:Title#anchor):
to one of this addon's pages, or else to a page of Leah's own reference (on Leah's wiki).

Code samples
------------
```leah          runs; if "Output:" and a code block follow, the output must match exactly
```leah check    only checked for mistakes (samples that need a board, a decoder, or the user's files)

They run with Leah (LEAH, else ~/Leah_Dev/target/all_addons/debug/leah or ~/Leah_Dev/target/debug/leah
when one is there, else `leah` on the PATH) and this addon as built (ADDON_FILE, else target/debug/libleah_addon_model_railroading.so: run
`cargo build` first), one at a time, each in a folder of its own, with no screen.
"""

import os
import re
import resource
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(os.environ.get("ADDON_ROOT", Path(__file__).resolve().parent.parent))
DOCS = ROOT / "docs"
WIKI_REPO = "model-railroading"
WIKI_URL = f"https://github.com/bmasters-dev/{WIKI_REPO}.wiki.git"
LEAH_WIKI = "https://github.com/bmasters-dev/leah-language/wiki/"
PUBLISHED = "Published from "
OVERVIEW = "Model railroading"


def leah_program():
    given = os.environ.get("LEAH")
    if given:
        return given
    # (Leah's development build with every addon first: the pages' samples may open windows.)
    for dev in (Path.home() / "Leah_Dev" / "target" / "all_addons" / "debug" / "leah", Path.home() / "Leah_Dev" / "target" / "debug" / "leah"):
        if dev.exists():
            return str(dev)
    found = shutil.which("leah")
    if not found:
        sys.exit("docs: there is no Leah to run the samples with (set LEAH)")
    return found


def addon_file():
    return Path(os.environ.get("ADDON_FILE", ROOT / "target" / "debug" / "libleah_addon_model_railroading.so"))


def pages(chosen=None):
    every = sorted(p for p in DOCS.glob("*.md") if p.name != "examples.md")
    if not chosen:
        return every
    picked = []
    for name in chosen:
        match = [p for p in every if p.stem == name or p.name == name or str(p) == name]
        if not match:
            sys.exit(f"docs: there is no page {name!r}")
        picked += match
    return picked


SAMPLE = re.compile(r"^```leah([^\n]*)\n(.*?)^```[ \t]*$", re.S | re.M)
OUTPUT = re.compile(r"\A\s*Output:\s*\n\s*```[^\n]*\n(.*?)^```[ \t]*$", re.S | re.M)
PROBLEM = re.compile(r"^(\w+_error\b|warning\b)", re.M)


def samples(text):
    """Each sample: (line number, flags, code, expected output or None)."""
    found = []
    for m in SAMPLE.finditer(text):
        out = OUTPUT.match(text[m.end():])
        found.append((text.count("\n", 0, m.start()) + 1, set(m.group(1).split()), m.group(2), out.group(1) if out else None))
    return found


def limits():
    # (A sample gone wrong can't take the computer's memory.)
    resource.setrlimit(resource.RLIMIT_AS, (4 << 30, 4 << 30))


def run_sample(leah, addons, flags, code, expected):
    """What went wrong, or None."""
    work = Path(tempfile.mkdtemp(prefix="mr-docs-"))
    try:
        script = work / "sample.leah"
        script.write_text(code)
        env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET")}
        # (No screen at all: a sample never opens a window on the user's own.)
        env.update({"LEAH_3RD_PARTY_ADDONS": str(addons), "GDK_BACKEND": "wayland", "WAYLAND_DISPLAY": str(work / "no-screen"),
                    "XDG_RUNTIME_DIR": str(work), "XDG_CONFIG_HOME": str(work / "config"), "XDG_DATA_HOME": str(work / "data"),
                    "XDG_CACHE_HOME": str(work / "cache"), "XDG_STATE_HOME": str(work / "state")})
        command = [leah, "--check", str(script)] if "check" in flags else [leah, str(script)]
        try:
            r = subprocess.run(command, cwd=work, env=env, capture_output=True, text=True, timeout=60, preexec_fn=limits)
        except subprocess.TimeoutExpired:
            return "it didn't end within 60 seconds"
        said = (r.stdout + r.stderr).rstrip()
        if r.returncode != 0 or PROBLEM.search(r.stderr):
            return f"it stopped (exit {r.returncode}):\n{said}"
        if expected is not None:
            got = "\n".join(l.rstrip() for l in r.stdout.rstrip().splitlines())
            want = "\n".join(l.rstrip() for l in expected.rstrip().splitlines())
            if got != want:
                return f"its output differs.\nexpected:\n{want}\ngot:\n{got}" + (f"\nand on its error output:\n{r.stderr.rstrip()}" if r.stderr.strip() else "")
        return None
    finally:
        shutil.rmtree(work, ignore_errors=True)


def slug(heading):
    """A heading's anchor, as GitHub makes it."""
    h = re.sub(r"[^\w\- ]", "", heading.strip().lower())
    return h.replace(" ", "-")


def anchors(text):
    prose = re.sub(r"^```.*?^```", "", text, flags=re.S | re.M)
    return {slug(h) for h in re.findall(r"^#{1,6} +(.+?)\s*$", prose, re.M)}


def links(text):
    prose = re.sub(r"^```.*?^```", "", text, flags=re.S | re.M)
    return re.findall(r"\[[^\]]+\]\((ref:[^)]*|#[^)]*)\)", prose)


def check_links(chosen):
    titles = {p.stem: p for p in pages()}
    problems = []
    for page in chosen:
        text = page.read_text()
        for target in links(text):
            if target.startswith("#"):
                if target[1:] not in anchors(text):
                    problems.append((page, f"{target}: no such heading on the page"))
                continue
            title, _, anchor = target[4:].partition("#")
            if title in titles:
                if anchor and anchor not in anchors(titles[title].read_text()):
                    problems.append((page, f"{target}: {title} has no such heading"))
            elif title[:1].islower() or title in ("Model railroading",):
                # (A lowercase title is an object's or a function's page: one of this addon's, or a typo.)
                problems.append((page, f"{target}: there is no page {title!r} here"))
        if not text.startswith(f"# {page.stem}\n"):
            problems.append((page, f"it should start with \"# {page.stem}\""))
    return problems


def check(args):
    leah = leah_program()
    built = addon_file()
    if not built.exists():
        sys.exit(f"docs: there is no {built}: run `cargo build` first")
    chosen = pages(args)
    jobs = [(p, line, flags, code, expected) for p in chosen for line, flags, code, expected in samples(p.read_text())]
    addons = Path(tempfile.mkdtemp(prefix="mr-docs-addons-"))
    failures = []
    try:
        shutil.copy(built, addons / "model_railroading.so")
        for page, line, flags, code, expected in jobs:
            result = run_sample(leah, addons, flags, code, expected)
            if result:
                failures.append((page, line, result))
    finally:
        shutil.rmtree(addons, ignore_errors=True)
    for page, line, message in failures:
        print(f"--- {page.name}:{line}\n{message}\n")
    problems = check_links(chosen)
    for page, message in problems:
        print(f"link in {page.name}: {message}")
    print(f"{len(jobs)} samples in {len(chosen)} pages: {len(failures)} failed; {len(problems)} link problems")
    return 1 if failures or problems else 0


# ---- wiki ----

def wiki_name(title):
    return title.replace("-", "‑").replace(" ", "-")


def to_wiki(text, titles):
    """A page as the wiki shows it: without its title line, its ref: links made wiki links (to this
    wiki's pages, or Leah's), outside code."""
    body = re.sub(r"\A# [^\n]*\n+", "", text)

    def link(m):
        title, _, anchor = m.group(2).partition("#")
        where = wiki_name(title) if title in titles else LEAH_WIKI + wiki_name(title)
        return f"[{m.group(1)}]({where}{'#' + anchor if anchor else ''})"

    parts = re.split(r"(^```.*?^```[ \t]*$)", body, flags=re.S | re.M)
    return "".join(part if part.startswith("```") else re.sub(r"\[([^\]]+)\]\(ref:([^)]+)\)", link, part) for part in parts)


def git(folder, *args, sign_in=(), check=True):
    return subprocess.run(["git", *sign_in, "-C", str(folder), *args], capture_output=True, text=True, check=check)


def summary(page):
    """A page's first sentence, for Home."""
    m = re.search(r"^## Description\s*\n+(.+?)(?<=[.:])(\s|$)", page.read_text(), re.M)
    return m.group(1).rstrip(":.") + "." if m else ""


def wiki(args):
    repo = ROOT.parent
    if git(repo, "status", "--porcelain", "--", str(DOCS)).stdout.strip():
        sys.exit("docs: docs/ has changes not committed: commit them first (only committed pages are published)")
    helper = git(repo, "config", "--get", "credential.https://github.com.helper", check=False).stdout.strip()
    sign_in = ["-c", f"credential.https://github.com.helper={helper}"] if helper else []
    folder = ROOT / "target" / "wiki" / f"{WIKI_REPO}.wiki"
    folder.parent.mkdir(parents=True, exist_ok=True)
    if not (folder / ".git").exists():
        r = subprocess.run(["git", *sign_in, "clone", "-q", WIKI_URL, str(folder)], capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"docs: the wiki can't be reached ({r.stderr.strip()}).\n"
                     f"A new wiki is made on the website: open https://github.com/bmasters-dev/{WIKI_REPO}/wiki, "
                     "\"Create the first page\", and save it as it is; then run this again.")
    else:
        git(folder, "fetch", "-q", sign_in=sign_in)
        git(folder, "reset", "-q", "--hard", "@{u}")
    # (Publishing goes one way: an edit made on the website since the last publish would be lost.)
    log = git(folder, "log", "--format=%s").stdout.splitlines()
    first_only = len(log) <= 1 and not any(l.startswith(PUBLISHED) for l in log)
    if log and not log[0].startswith(PUBLISHED) and not first_only and "--force" not in args:
        sys.exit(f"docs: the wiki was edited on the website since it was last published ({log[0]!r}): "
                 "bring that edit into docs/ by hand, then run this again with --force")
    for key in ["user.name", "user.email"]:
        git(folder, "config", key, git(repo, "config", "--get", key).stdout.strip())

    every = pages()
    titles = {p.stem for p in every}
    files = {f"{wiki_name(p.stem)}.md": to_wiki(p.read_text(), titles) for p in every}
    objects = [p for p in every if p.stem[:1].islower()]
    functions = [p for p in every if p.stem[:1].isupper() and p.stem != OVERVIEW]
    version = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M).group(1)
    home = ["The **model_railroading** addon for [Leah](https://github.com/bmasters-dev/leah-language): Leah runs "
            "model trains as a DCC command station. It is a 3rd-party addon, installed by its address:", "",
            "```", f"leah --install-addon https://github.com/bmasters-dev/{WIKI_REPO}", "```", "",
            f"These pages are for version {version}. Leah's own language is in the "
            f"[Leah Language Reference]({LEAH_WIKI}Home).", "",
            "## Start here", "", f"- [{OVERVIEW}]({wiki_name(OVERVIEW)}): {summary(DOCS / (OVERVIEW + '.md'))}", "",
            "## Functions", ""]
    home += [f"- [{p.stem}]({wiki_name(p.stem)})" for p in functions]
    home += ["", "## Objects", ""]
    home += [f"- [{p.stem}]({wiki_name(p.stem)}): {summary(p)}" for p in objects]
    files["Home.md"] = to_wiki("\n".join(home) + "\n", titles)
    files["_Header.md"] = f"**[Home](Home)** · [{OVERVIEW}]({wiki_name(OVERVIEW)}) · [Leah Language Reference]({LEAH_WIKI}Home)\n"
    sidebar = ["**[Home](Home)**", "", f"[{OVERVIEW}]({wiki_name(OVERVIEW)})", "", "**Functions**", ""]
    sidebar += [f"- [{p.stem}]({wiki_name(p.stem)})" for p in functions]
    sidebar += ["", "**Objects**", ""] + [f"- [{p.stem}]({wiki_name(p.stem)})" for p in objects]
    files["_Sidebar.md"] = "\n".join(sidebar) + "\n"

    for old in folder.rglob("*.md"):
        if ".git" not in old.parts:
            old.unlink()
    for name, text in files.items():
        (folder / name).write_text(text)
    git(folder, "add", "-A")
    if git(folder, "diff", "--cached", "--quiet", check=False).returncode == 0:
        print("the wiki is up to date")
        return 0
    head = git(repo, "rev-parse", "--short", "HEAD").stdout.strip()
    git(folder, "commit", "-q", "-m", f"{PUBLISHED}model-railroading {head} (addon {version})")
    if "--no-push" in args:
        print(f"committed in {folder}, not pushed (--no-push)")
        return 0
    r = subprocess.run(["git", *sign_in, "-C", str(folder), "push", "-q"], capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"docs: couldn't push: {r.stderr.strip()}")
    print(f"published {len(every)} pages to https://github.com/bmasters-dev/{WIKI_REPO}/wiki")
    return 0


def help_file(args):
    """The pages' help as Leah reads it from `<name>.help.json` beside the addon's library (Leah's
    docs/spec/addons.md, "A 3rd-party addon's help"), made by Leah's own reader of reference pages
    (LEAH_TOOLS, else ~/Leah_Dev/tools: the kit will carry it once published, leah-language#720)."""
    import json
    to = Path(args[args.index("--to") + 1]) if "--to" in args else ROOT / "target" / "model_railroading.help.json"
    tools = Path(os.environ.get("LEAH_TOOLS", Path.home() / "Leah_Dev" / "tools"))
    if not (tools / "make_ai_guide.py").exists():
        sys.exit(f"docs: Leah's reader of reference pages isn't in {tools} (set LEAH_TOOLS)")
    sys.path.insert(0, str(tools))
    import make_ai_guide
    made = make_ai_guide.addon_help(pages(), "model_railroading")
    to.parent.mkdir(parents=True, exist_ok=True)
    to.write_text(json.dumps(made, ensure_ascii=False, sort_keys=True, indent=0) + "\n", encoding="utf-8")
    print(f"wrote {to}: {len(made['functions'])} functions, {len(made['members'])} objects", file=sys.stderr)
    return 0


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in ("check", "wiki", "help"):
        sys.exit(__doc__)
    rest = [a for a in sys.argv[2:] if not a.startswith("--")]
    if sys.argv[1] == "help":
        sys.exit(help_file(sys.argv[2:]))
    sys.exit(check(rest) if sys.argv[1] == "check" else wiki(sys.argv[2:]))
