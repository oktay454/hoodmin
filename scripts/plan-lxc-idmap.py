#!/usr/bin/python3
"""Read-only report for a separately approved LXC ownership migration."""
import errno
import json
import os
from pathlib import Path
import re
import sys

name=sys.argv[1]
assert name in ('hoodmin','vh01')
base=Path('/var/lib/lxc')/name
mapping=re.findall(r'^lxc\.idmap\s*=\s*([ug])\s+0\s+(\d+)\s+65536\s*$',(base/'config').read_text(),re.M)
assert sorted(mapping)==[('g','100000'),('u','100000')]
conflicts=[]
for filename in ('subuid','subgid'):
	for row in (Path('/etc')/filename).read_text().splitlines():
		user,start,count=row.split(':');start,count=int(start),int(count)
		if start<265536 and start+count>200000:conflicts.append(dict(file=filename,user=user,start=start,count=count))
seen=set();unexpected=0;capabilities=0;size=0
for directory,dirs,files in os.walk(base/'rootfs',followlinks=False):
	for path in [Path(directory),*(Path(directory)/x for x in dirs+files)]:
		metadata=path.lstat();inode=(metadata.st_dev,metadata.st_ino)
		if inode in seen:continue
		seen.add(inode);size+=metadata.st_size
		if not (100000<=metadata.st_uid<165536 and 100000<=metadata.st_gid<165536):unexpected+=1
		try:
			if os.getxattr(path,'security.capability',follow_symlinks=False):capabilities+=1
		except OSError as error:
			if error.errno not in (errno.ENODATA,errno.ENOTSUP):raise
print(json.dumps(dict(container=name,current_base=100000,proposed_base=200000,unique_inodes=len(seen),logical_bytes=size,unexpected_ownership=unexpected,file_capabilities=capabilities,new_range_conflicts=conflicts),indent='\t'))
