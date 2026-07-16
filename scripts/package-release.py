#!/usr/bin/env python3
import gzip
import hashlib
import pathlib
import sys
import tarfile
import zipfile

if len(sys.argv) not in (4, 5):
    raise SystemExit("usage: package-release.py TARGET VERSION BINARY [OUTPUT]")

target, version, binary_arg = sys.argv[1:4]
binary = pathlib.Path(binary_arg)
output = pathlib.Path(sys.argv[4] if len(sys.argv) == 5 else "target/distrib")
if not binary.is_file():
    raise SystemExit(f"release binary does not exist: {binary}")
output.mkdir(parents=True, exist_ok=True)
name = f"dexdeck-{version}-{target}"
epoch = 1767225600  # 2026-01-01T00:00:00Z
assets = [
    (pathlib.Path("README.md"), "README.md", 0o644),
    (pathlib.Path("LICENSE"), "LICENSE", 0o644),
    (pathlib.Path("NOTICE"), "NOTICE", 0o644),
    (pathlib.Path("man/dexdeck.1"), "man/man1/dexdeck.1", 0o644),
    (pathlib.Path("completions/dexdeck.bash"), "completions/dexdeck.bash", 0o644),
    (pathlib.Path("completions/_dexdeck"), "completions/_dexdeck", 0o644),
    (pathlib.Path("completions/dexdeck.fish"), "completions/dexdeck.fish", 0o644),
]
windows = "windows" in target
assets.append((binary, "dexdeck.exe" if windows else "dexdeck", 0o755))

if windows:
    archive = output / f"{name}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as package:
        for source, destination, mode in sorted(assets, key=lambda item: item[1]):
            info = zipfile.ZipInfo(f"{name}/{destination}", (2026, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (0o100000 | mode) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            package.writestr(info, source.read_bytes())
else:
    archive = output / f"{name}.tar.gz"
    with archive.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as package:
                for source, destination, mode in sorted(assets, key=lambda item: item[1]):
                    info = package.gettarinfo(str(source), f"{name}/{destination}")
                    info.mtime = epoch
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    info.mode = mode
                    with source.open("rb") as contents:
                        package.addfile(info, contents)

digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_name(archive.name + ".sha256").write_text(
    f"{digest}  {archive.name}\n", encoding="utf-8", newline="\n"
)
