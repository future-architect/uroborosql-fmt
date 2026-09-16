#!/bin/sh
set -eu
mkdir -p /tmp/catalog-tls
cd /tmp/catalog-tls
if [ ! -f ca.pem ]; then
  openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj /CN=catalog-test-ca \
    -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign,cRLSign \
    -keyout ca-key.pem -out ca.pem
fi
openssl req -new -newkey rsa:2048 -nodes -subj /CN=catalog-test \
  -keyout key.pem -out server.csr
printf '%s\n' basicConstraints=critical,CA:FALSE \
  keyUsage=critical,digitalSignature,keyEncipherment extendedKeyUsage=serverAuth \
  "subjectAltName=$1" > extensions
openssl x509 -req -days 2 -in server.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -extfile extensions -out cert.pem
chown postgres:postgres key.pem
chmod 600 key.pem
psql -X -U postgres -v ON_ERROR_STOP=1 <<'SQL'
ALTER SYSTEM SET ssl = 'on';
ALTER SYSTEM SET ssl_cert_file = '/tmp/catalog-tls/cert.pem';
ALTER SYSTEM SET ssl_key_file = '/tmp/catalog-tls/key.pem';
SELECT pg_reload_conf();
SQL
