#!/usr/bin/python3
"""Prepare a stopped clone; never modify the original template."""
import os
from pathlib import Path
import subprocess
import sys

name,address=sys.argv[1:]
assert name in ('hoodmin','vh01') and address in ('10.0.3.10','10.0.3.20')
assert subprocess.check_output(['lxc-info','-n',name,'-sH'],text=True).strip()=='STOPPED'
base=Path('/var/lib/lxc')/name
os.chown(base,0,100000)
os.chmod(base,0o710)
config=base/'config'
text=config.read_text()
assert 'lxc.idmap' not in text,'Already prepared; refusing to shift ownership twice.'
(base/'config.before-hoodmin').write_text(text)
lines=[line for line in text.splitlines() if not line.startswith(('lxc.mount.entry = /usr/local/share/certificates','lxc.apparmor.allow_nesting','lxc.start.auto'))]
lines.extend(['lxc.apparmor.allow_nesting = 0','lxc.idmap = u 0 100000 65536','lxc.idmap = g 0 100000 65536','lxc.start.auto = 1'])
config.write_text('\n'.join(lines)+'\n')
root=base/'rootfs'
(root/'etc/network/interfaces').write_text(f'auto lo\niface lo inet loopback\n\nauto eth0\niface eth0 inet static\n\taddress {address}/24\n\tgateway 10.0.3.1\n\tdns-nameservers 10.0.3.1\n')
(root/'etc/resolv.conf').unlink(missing_ok=True)
(root/'etc/resolv.conf').write_text('nameserver 10.0.3.1\n')
(root/'etc/hostname').write_text(name+'\n')
(root/'etc/hosts').write_text(f'127.0.0.1 localhost\n127.0.1.1 {name}\n')
(root/'etc/machine-id').write_text('')
for entry in (root/'etc/ssh').glob('ssh_host_*'):
	entry.unlink()
# Shift each inode once, retaining the template's relative users/groups.
seen=set()
for directory,dirs,files in os.walk(root,followlinks=False):
	for path in [Path(directory),*(Path(directory)/x for x in dirs+files)]:
		stat=path.lstat();inode=(stat.st_dev,stat.st_ino)
		if inode in seen:continue
		seen.add(inode)
		assert stat.st_uid<65536 and stat.st_gid<65536,path
		os.lchown(path,stat.st_uid+100000,stat.st_gid+100000)
		if not path.is_symlink():os.chmod(path,stat.st_mode & 0o7777)
for file in ('subuid','subgid'):
	path=Path('/etc')/file;text=path.read_text()
	if 'root:100000:65536' not in text:path.write_text(text+'root:100000:65536\n')
print(f'{name}: unprivileged UID/GID map, fixed IP and separate credentials prepared.')
