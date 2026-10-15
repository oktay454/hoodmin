#!/bin/bash
set -euo pipefail

TLS_DIZINI="/etc/vpnhood-manager/tls"
install -d -m 0700 "${TLS_DIZINI}"
umask 077
if [ ! -f "${TLS_DIZINI}/ca.crt" ]
then
	openssl req -x509 -newkey rsa:3072 -sha256 -nodes -days 30 -addext 'basicConstraints=critical,CA:TRUE,pathlen:0' -addext 'keyUsage=critical,keyCertSign,cRLSign' -subj '/CN=VpnHood Test CA' -keyout "${TLS_DIZINI}/ca.key" -out "${TLS_DIZINI}/ca.crt" 2>"${TLS_DIZINI}/openssl.log"
	openssl req -newkey rsa:3072 -sha256 -nodes -subj '/CN=yapayzeka' -keyout "${TLS_DIZINI}/manager.key" -out "${TLS_DIZINI}/manager.csr" 2>>"${TLS_DIZINI}/openssl.log"
	printf '%s\n' 'subjectAltName=IP:10.0.3.236,DNS:yapayzeka' 'extendedKeyUsage=serverAuth' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' >"${TLS_DIZINI}/manager.ext"
	openssl x509 -req -in "${TLS_DIZINI}/manager.csr" -CA "${TLS_DIZINI}/ca.crt" -CAkey "${TLS_DIZINI}/ca.key" -CAcreateserial -days 30 -sha256 -extfile "${TLS_DIZINI}/manager.ext" -out "${TLS_DIZINI}/manager.crt" 2>>"${TLS_DIZINI}/openssl.log"
fi
if ! openssl x509 -in "${TLS_DIZINI}/ca.crt" -noout -ext keyUsage 2>/dev/null | grep -q 'Certificate Sign'
then
	openssl req -x509 -key "${TLS_DIZINI}/ca.key" -sha256 -days 30 -addext 'basicConstraints=critical,CA:TRUE,pathlen:0' -addext 'keyUsage=critical,keyCertSign,cRLSign' -subj '/CN=VpnHood Test CA' -out "${TLS_DIZINI}/ca.crt"
	printf '%s\n' 'subjectAltName=IP:10.0.3.236,DNS:yapayzeka' 'extendedKeyUsage=serverAuth' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' >"${TLS_DIZINI}/manager.ext"
	openssl x509 -req -in "${TLS_DIZINI}/manager.csr" -CA "${TLS_DIZINI}/ca.crt" -CAkey "${TLS_DIZINI}/ca.key" -CAcreateserial -days 30 -sha256 -extfile "${TLS_DIZINI}/manager.ext" -out "${TLS_DIZINI}/manager.crt" 2>>"${TLS_DIZINI}/openssl.log"
fi
cat >/etc/nginx/conf.d/vpnhood-manager-test.conf <<'NGINX'
server {
	listen 10.0.3.236:3443 ssl;
	listen 127.0.1.1:3443 ssl;
	server_name yapayzeka;
	ssl_certificate /etc/vpnhood-manager/tls/manager.crt;
	ssl_certificate_key /etc/vpnhood-manager/tls/manager.key;
	ssl_protocols TLSv1.2 TLSv1.3;
	client_max_body_size 512k;
	location /events {
		proxy_pass http://127.0.0.1:3080;
		proxy_buffering off;
		proxy_cache off;
		proxy_read_timeout 1h;
		proxy_set_header Host $host;
		proxy_set_header X-Real-IP $remote_addr;
	}
	location / {
		proxy_pass http://127.0.0.1:3080;
		proxy_set_header Host $host;
		proxy_set_header X-Real-IP $remote_addr;
		proxy_set_header X-Forwarded-Proto https;
		proxy_next_upstream off;
	}
}
NGINX
nginx -t
systemctl reload nginx
curl --fail --silent --retry 10 --retry-connrefused --retry-delay 1 --cacert "${TLS_DIZINI}/ca.crt" https://10.0.3.236:3443/healthz
