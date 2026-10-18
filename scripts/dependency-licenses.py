#!/usr/bin/python3
"""Record dependency license declarations without leaking local build paths."""
import json
import sys
metadata=json.load(sys.stdin)
packages=[dict(name=p['name'],version=p['version'],license=p['license'],repository=p.get('repository'),source=p.get('source')) for p in metadata['packages'] if p.get('source')]
json.dump(sorted(packages,key=lambda p:(p['name'],p['version'])),sys.stdout,ensure_ascii=False,indent='\t')
print()
