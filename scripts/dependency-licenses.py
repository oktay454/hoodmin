#!/usr/bin/python3
"""Install Cargo license declarations and upstream notices without build paths."""
import json
from pathlib import Path
import shutil
import sys


def is_notice(path):
	return path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")) or path.suffix == ".license"


def notices(package, destination):
	root = Path(package["manifest_path"]).parent
	files = {path for path in root.iterdir() if path.is_file() and is_notice(path)}
	licenses = root / "LICENSES"
	if licenses.is_dir():
		files.update(path for path in licenses.rglob("*") if path.is_file())
	if package["license_file"]:
		files.add(root / package["license_file"])
	if not files:
		raise SystemExit(f'No license notice found for {package["name"]} {package["version"]}')
	copied = []
	for path in sorted(files):
		relative = path.relative_to(root)
		if ".." in relative.parts or not path.resolve().is_relative_to(root.resolve()):
			raise SystemExit(f'Invalid license path for {package["name"]}')
		target = destination / relative
		target.parent.mkdir(parents=True, exist_ok=True)
		shutil.copyfile(path, target)
		copied.append(target.as_posix())
	return copied


directory = Path(sys.argv[1])
metadata = json.load(sys.stdin)
resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
packages = []
for package in metadata["packages"]:
	if not package["source"] or package["id"] not in resolved:
		continue
	name, version = package["name"], package["version"]
	copied = notices(package, directory / "dependency-notices" / f"{name}-{version}")
	entry = {field: package[field] for field in ("name", "version", "license", "repository", "source")}
	entry["notices"] = [Path(path).relative_to(directory).as_posix() for path in copied]
	packages.append(entry)
(directory / "dependency-licenses.json").write_text(
	json.dumps(sorted(packages, key=lambda entry: (entry["name"], entry["version"])), indent=2) + "\n",
	encoding="utf-8",
)
