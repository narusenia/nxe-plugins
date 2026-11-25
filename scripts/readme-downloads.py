"""Rewrite the README's download table from the published releases.

One row per series in `series.toml`, pointing at that series' newest published
release (drafts are skipped) and at the files it carries: the installer when
there is one, and the series zip. Run it after publishing a release
(`mise run readme:downloads`); it needs the GitHub CLI, signed in.

The table sits between the `downloads:start` and `downloads:end` markers in
`README.md`; everything else in the file is left alone.
"""

import json
import re
import subprocess
import tomllib

REPO = "narusenia/nxe-plugins"
BASE = f"https://github.com/{REPO}/releases"
START, END = "<!-- downloads:start -->", "<!-- downloads:end -->"


def gh(*args):
    return json.loads(subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout)


def newest(releases, key):
    """The newest published release of a series. `gh` lists newest first."""
    pattern = re.compile(rf"^{re.escape(key)}-\d{{4}}\.")
    return next((r["tagName"] for r in releases if not r["isDraft"] and pattern.match(r["tagName"])), None)


def links(tag, assets, platform, installer):
    found = []
    for label, name in (("installer", f"nxe-{tag}-{platform}-installer.{installer}"), ("zip", f"nxe-{tag}-{platform}.zip")):
        if name in assets:
            found.append(f"[{label}]({BASE}/download/{tag}/{name})")
    return " · ".join(found) or "—"


def table(series, releases):
    rows = [
        "| Series | What it is | Latest | macOS | Windows | All releases |",
        "|---|---|---|---|---|---|",
    ]
    for key, entry in series.items():
        name = entry["name"]
        tag = newest(releases, key)
        every = f"[{name}]({BASE}?q={key}-&expanded=true)"
        if tag is None:
            rows.append(f"| [**{name}**](#{key}) | {entry['summary']} | not released | — | — | {every} |")
            continue
        assets = {a["name"] for a in gh("release", "view", tag, "--repo", REPO, "--json", "assets")["assets"]}
        rows.append(
            f"| [**{name}**](#{key}) | {entry['summary']} | [`{tag}`]({BASE}/tag/{tag}) "
            f"| {links(tag, assets, 'macos', 'pkg')} | {links(tag, assets, 'windows', 'exe')} | {every} |"
        )
    return "\n".join(rows)


def main():
    series = tomllib.load(open("series.toml", "rb"))
    releases = gh("release", "list", "--repo", REPO, "--limit", "200", "--json", "tagName,isDraft")
    readme = open("README.md").read()
    before, rest = readme.split(START, 1)
    _, after = rest.split(END, 1)
    open("README.md", "w").write(f"{before}{START}\n{table(series, releases)}\n{END}{after}")


if __name__ == "__main__":
    main()
