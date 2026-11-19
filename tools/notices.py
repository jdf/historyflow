#!/usr/bin/env python3
"""Writes web/licenses.txt: the license notices of every crate that is
compiled into the viewer or the command, which the About box links to.

  tools/notices.py           write the file
  tools/notices.py --check   fail if the file is not what would be written

The crates are those `cargo tree` lists as normal dependencies (nothing
that only runs at build time) of the viewer for wasm and of the command
for each platform it is released on. Their license files are read from
cargo's copies of their sources, so the crates must have been fetched
(any build does that).

Where a crate offers a choice of licenses, MIT is taken if it is one of
them. A crate whose license files cannot be matched to its declared
license stops the script: add the answer to this script by hand rather
than ship a guess.
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "web" / "licenses.txt"

BUILDS = [
    ("historyflow-web", "wasm32-unknown-unknown"),
    ("historyflow-cli", "aarch64-apple-darwin"),
    ("historyflow-cli", "x86_64-apple-darwin"),
    ("historyflow-cli", "x86_64-unknown-linux-musl"),
]

# In order of preference, each with a phrase its text always contains.
KNOWN = [
    ("MIT", "Permission is hereby granted, free of charge"),
    ("Apache-2.0", "Apache License"),
    ("Zlib", "This software is provided 'as-is'"),
    ("Unicode-3.0", "UNICODE LICENSE V3"),
    ("ISC", "Permission to use, copy, modify, and/or distribute this software"),
    ("BSD-3-Clause", "Redistribution and use in source and binary forms"),
    ("CDLA-Permissive-2.0", "Community Data License Agreement"),
]

RUST = """Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE."""


def cargo(*args):
    return subprocess.run(
        ["cargo", *args], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def linked():
    """(name, version) of every crate from elsewhere that is compiled in."""
    found = set()
    for package, target in BUILDS:
        tree = cargo(
            "tree", "--locked", "-p", package, "--target", target,
            "-e", "normal,no-proc-macro", "--prefix", "none", "-f", "{p}",
        )  # fmt: skip
        for line in tree.splitlines():
            name, version, *rest = line.split()
            # One of ours has its path after the version.
            if not any(part.startswith("(/") for part in rest):
                found.add((name, version.removeprefix("v")))
    return sorted(found)


def chosen(expression):
    """The licenses taken from a crate's license expression: every part
    joined by AND, and of each choice the one we prefer."""
    taken = []
    for part in re.split(r"\s+AND\s+", expression):
        offered = re.split(r"\s+OR\s+|/", part.strip("() "))
        pick = next((name for name, _ in KNOWN if name in offered), None)
        if pick is None:
            sys.exit(f"notices: no license we know in {expression!r}")
        taken.append(pick)
    return taken


def texts(directory, licenses, crate):
    """The license files in a crate's sources that state `licenses`."""
    files = sorted(
        path
        for path in directory.iterdir()
        if path.is_file()
        and re.match(r"(?i)(licen[sc]e|copying|notice)", path.name)
    )
    out = []
    # ring is made of parts under several licenses, each in its own file.
    if crate.startswith("ring "):
        out += files
    for wanted in licenses:
        phrase = dict(KNOWN)[wanted]
        matching = [
            path for path in files if phrase in path.read_text(errors="replace")
        ]
        if not matching:
            sys.exit(f"notices: {crate} has no {wanted} license file")
        out += matching
        # Apache-2.0 asks that a NOTICE file be passed on too.
        if wanted == "Apache-2.0":
            out += [p for p in files if p.name.upper().startswith("NOTICE")]
    seen = []
    for path in out:
        text = path.read_text(errors="replace").strip()
        text = "\n".join(line.rstrip() for line in text.splitlines())
        if text not in seen:
            seen.append(text)
    return seen


def notices():
    packages = {
        (p["name"], p["version"]): p
        for p in json.loads(cargo("metadata", "--locked", "--format-version", "1"))[
            "packages"
        ]
    }
    # Each distinct text once, with the crates it covers.
    covered = {}
    for crate in linked():
        package = packages[crate]
        directory = pathlib.Path(package["manifest_path"]).parent
        label = f"{crate[0]} {crate[1]}"
        for text in texts(directory, chosen(package["license"]), label):
            covered.setdefault(text, []).append(label)

    rule = "=" * 72
    parts = [
        "history flow is built with the software listed here, each under\n"
        "the license that follows its name. history flow's own license is\n"
        "the first.",
        f"{rule}\nhistory flow\n{rule}\n\n" + (ROOT / "LICENSE").read_text().strip(),
        f"{rule}\nThe Rust standard library\n{rule}\n\n"
        "Copyright (c) The Rust Project Contributors\n\n" + RUST,
    ]
    for text, crates in sorted(covered.items(), key=lambda item: item[1]):
        parts.append(f"{rule}\n" + "\n".join(crates) + f"\n{rule}\n\n{text}")
    return "\n\n\n".join(parts) + "\n"


def main():
    wanted = notices()
    if sys.argv[1:] == ["--check"]:
        if not OUT.exists() or OUT.read_text() != wanted:
            sys.exit("web/licenses.txt is out of date: run tools/notices.py")
    else:
        OUT.write_text(wanted)


if __name__ == "__main__":
    main()
