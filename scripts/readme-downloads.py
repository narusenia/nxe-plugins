"""Rewrite the README's download table, and `site/downloads.json`, from the
published releases.

One row per series in `series.toml`, pointing at that series' newest published
release (drafts are skipped; betas count) and at the files it carries: the
installer when there is one, and the series zip. Run it after publishing a
release (`mise run readme:downloads`); it needs the GitHub CLI, signed in.

The table sits between the `downloads:start` and `downloads:end` markers in
`README.md`; everything else in the file is left alone.

`site/downloads.json` says the same for the site and anything else that wants
the latest download links: per series its tag, whether it is a beta, the
installer and zip per platform, and every plugin's version and zips. The
Pages workflow runs `--json-only` before each deploy, so the site never waits
for a commit.
"""

import json
import re
import subprocess
import sys
import tomllib

REPO = "nxe-plugins/nxe-plugins"
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
    # Linux has no installer, only the zip.
    for label, name in (("installer", f"nxe-{tag}-{platform}-installer.{installer}"), ("zip", f"nxe-{tag}-{platform}.zip")):
        if name in assets:
            found.append(f"[{label}]({BASE}/download/{tag}/{name})")
    return " · ".join(found) or "—"


def table(series, releases):
    rows = [
        "| Series | What it is | Latest | macOS | Windows | Linux | All releases |",
        "|---|---|---|---|---|---|---|",
    ]
    for key, entry in series.items():
        name = entry["name"]
        tag = newest(releases, key)
        every = f"[{name}]({BASE}?q={key}-&expanded=true)"
        if tag is None:
            rows.append(f"| [**{name}**](#{key}) | {entry['summary']} | not released | — | — | — | {every} |")
            continue
        assets = {a["name"] for a in gh("release", "view", tag, "--repo", REPO, "--json", "assets")["assets"]}
        rows.append(
            f"| [**{name}**](#{key}) | {entry['summary']} | [`{tag}`]({BASE}/tag/{tag}) "
            f"| {links(tag, assets, 'macos', 'pkg')} | {links(tag, assets, 'windows', 'exe')} "
            f"| {links(tag, assets, 'linux', None)} | {every} |"
        )
    return "\n".join(rows)


PLUGIN_ZIP = re.compile(r"^nxe-(?P<plugin>[a-z0-9-]+)-(?P<version>\d+\.\d+\.\d+)-(?P<platform>macos|windows|linux)\.zip$")


def manifest(series, releases):
    """Every series' newest published release, as `site/downloads.json` holds it."""
    out = {}
    for key, entry in series.items():
        tag = newest(releases, key)
        if tag is None:
            out[key] = {"name": entry["name"], "summary": entry["summary"], "tag": None}
            continue
        release = gh("release", "view", tag, "--repo", REPO, "--json", "assets,isPrerelease,publishedAt")
        assets = {a["name"] for a in release["assets"]}
        url = lambda name: f"{BASE}/download/{tag}/{name}" if name in assets else None
        plugins = {}
        for name in sorted(assets):
            match = PLUGIN_ZIP.match(name)
            if match:
                plugin = plugins.setdefault(match["plugin"], {"version": match["version"]})
                plugin[match["platform"]] = url(name)
        out[key] = {
            "name": entry["name"],
            "summary": entry["summary"],
            "tag": tag,
            "beta": release["isPrerelease"] or "-beta." in tag,
            "published": release["publishedAt"],
            "page": f"{BASE}/tag/{tag}",
            "macos": {"installer": url(f"nxe-{tag}-macos-installer.pkg"), "zip": url(f"nxe-{tag}-macos.zip")},
            "windows": {"installer": url(f"nxe-{tag}-windows-installer.exe"), "zip": url(f"nxe-{tag}-windows.zip")},
            "linux": {"zip": url(f"nxe-{tag}-linux.zip")},
            "plugins": plugins,
        }
    return out


def main():
    series = tomllib.load(open("series.toml", "rb"))
    releases = gh("release", "list", "--repo", REPO, "--limit", "200", "--json", "tagName,isDraft")
    with open("site/downloads.json", "w") as out:
        json.dump(manifest(series, releases), out, indent=2)
        out.write("\n")
    if "--json-only" in sys.argv:
        return
    readme = open("README.md").read()
    before, rest = readme.split(START, 1)
    _, after = rest.split(END, 1)
    open("README.md", "w").write(f"{before}{START}\n{table(series, releases)}\n{END}{after}")


if __name__ == "__main__":
    main()
