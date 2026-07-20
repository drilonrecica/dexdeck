#!/usr/bin/env python3
import argparse
import hashlib
import pathlib
import re
import tempfile

TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
)


def parse_checksums(path: pathlib.Path) -> dict[str, str]:
    checksums = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})\s+\*?(.+)", line.strip())
        if match:
            checksums[pathlib.Path(match.group(2)).name] = match.group(1)
    return checksums


def generate(template: pathlib.Path, checksums: pathlib.Path, version: str) -> str:
    source = template.read_text(encoding="utf-8").replace("@VERSION@", version)
    values = parse_checksums(checksums)
    for target in TARGETS:
        archive = f"dexdeck-{version}-{target}.tar.gz"
        if archive not in values:
            raise ValueError(f"missing checksum for {archive}")
        token = "@SHA_" + target.upper().replace("-", "_") + "@"
        source = source.replace(token, values[archive])
    if re.search(r"@[A-Z0-9_]+@", source):
        raise ValueError("unresolved formula token")
    return source


def self_test(template: pathlib.Path) -> None:
    version = "9.8.7"
    with tempfile.TemporaryDirectory() as directory:
        checksums = pathlib.Path(directory) / "SHA256SUMS"
        lines = []
        for target in TARGETS:
            name = f"dexdeck-{version}-{target}.tar.gz"
            digest = hashlib.sha256(name.encode()).hexdigest()
            lines.append(f"{digest}  {name}")
        checksums.write_text("\n".join(lines) + "\n", encoding="utf-8")
        formula = generate(template, checksums, version)
        assert formula.count("sha256") == 4
        assert "@" not in formula


parser = argparse.ArgumentParser()
parser.add_argument("--template", type=pathlib.Path, default=pathlib.Path("packaging/homebrew/dexdeck.rb.in"))
parser.add_argument("--checksums", type=pathlib.Path)
parser.add_argument("--version")
parser.add_argument("--output", type=pathlib.Path)
parser.add_argument("--self-test", action="store_true")
arguments = parser.parse_args()

if arguments.self_test:
    self_test(arguments.template)
else:
    if not all((arguments.checksums, arguments.version, arguments.output)):
        parser.error("--checksums, --version, and --output are required")
    result = generate(arguments.template, arguments.checksums, arguments.version)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(result, encoding="utf-8", newline="\n")
