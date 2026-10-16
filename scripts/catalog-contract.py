#!/usr/bin/env python3
"""Read upstream declarations only; never read deployment settings or credentials."""
import argparse
import hashlib
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
classes = {}
paths = list((args.source / "src/Core/VpnHood.Core.Server.Access/Configurations").glob("*.cs"))
paths += list((args.source / "src/Core/VpnHood.Core.Server.Access/Messaging").glob("*.cs"))
paths += list((args.source / "src/Core/VpnHood.Core.Common/Messaging").glob("*.cs"))
paths += list((args.source / "src/Core/VpnHood.Core.Common/Tokens").glob("*.cs"))
paths += list((args.source / "src/Core/VpnHood.Core.Server.Access").glob("*.cs"))
paths += [args.source / "src/Apps/Server/AppSettings.cs"]
for path in sorted(paths):
	text = path.read_text(encoding="utf-8-sig")
	match = re.search(r"public (?:sealed )?class (\w+)", text)
	if not match:
		continue
	fields = []
	for prop in re.finditer(r"((?:\s*\[[^\]]*\])*)\s*public\s+(?:required\s+)?([\w<>,?\[\].]+)\s+(\w+)\s*\{\s*get;\s*(?:set|init);", text):
		attributes, kind, name = prop.groups()
		if re.search(r"\[JsonIgnore\s*\]", attributes):
			continue
		naming = re.search(r'JsonPropertyName\("([^"\n]+)"\)', attributes)
		fields.append({"name": naming.group(1) if naming else name[0].lower() + name[1:], "type": kind, "line": text[:prop.start()].count("\n") + 1})
	classes[match.group(1)] = {"source": str(path.relative_to(args.source)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "fields": fields}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps({"note": "HTTP camelCase; explicit JsonPropertyName preserved. Inheritance is listed in separate classes. Derived getters are excluded.", "classes": classes}, ensure_ascii=False, indent=2) + "\n")
print(f"Catalogued {len(classes)} classes; no deployment files read.")
