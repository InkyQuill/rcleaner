"""Package one native executable and its notices for a GitHub release."""
import argparse
import hashlib
from pathlib import Path
import tarfile
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target")
    parser.add_argument("--tag")
    args = parser.parse_args()
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    if args.tag and args.tag != f"v{version}":
        parser.error(f"tag {args.tag} does not match Cargo.toml v{version}")
    name = f"rcleaner-v{version}-{args.target}"
    executable = "rcleaner.exe" if "windows" in args.target else "rcleaner"
    files = {
        executable: Path("target") / args.target / "release" / executable,
        "README.md": Path("README.md"),
        "LICENSE": Path("LICENSE"),
        "THIRD_PARTY.md": Path("THIRD_PARTY.md"),
        "licenses/oxicleaner-MIT.txt": Path("licenses/oxicleaner-MIT.txt"),
        "vendor/cargo-sweep/LICENSE": Path("vendor/cargo-sweep/LICENSE"),
        "vendor/cargo-sweep/README.md": Path("vendor/cargo-sweep/README.md"),
    }
    for source in files.values():
        if not source.is_file():
            parser.error(f"missing release input: {source}")
    Path("dist").mkdir(exist_ok=True)
    if "windows" in args.target:
        archive = Path("dist") / f"{name}.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for destination, source in files.items():
                output.write(source, f"{name}/{destination}")
    else:
        archive = Path("dist") / f"{name}.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            for destination, source in files.items():
                output.add(source, arcname=f"{name}/{destination}")
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{checksum}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
