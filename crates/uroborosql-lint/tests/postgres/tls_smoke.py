#!/usr/bin/env python3
"""Verify the production client's TLS handshake without installing PostgreSQL."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import socket
import ssl
import struct
import subprocess

HERE = Path(__file__).resolve().parent
CERTS = HERE / 'tls-smoke'
ROOT = HERE.parents[3]


def receive(stream, size):
    data = b''
    while len(data) < size:
        part = stream.recv(size - len(data))
        if not part:
            raise EOFError('peer closed before complete protocol message')
        data += part
    return data


def serve(listener, certificate):
    connection, _ = listener.accept()
    with connection:
        connection.settimeout(10)
        assert receive(connection, 8) == struct.pack('!II', 8, 80877103), 'missing SSLRequest'
        if certificate is None:
            connection.sendall(b'N')
            try:
                assert connection.recv(1) == b'', 'client continued without TLS'
            except ConnectionResetError:
                pass
            return False
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(CERTS / certificate, CERTS / 'server.key')
        connection.sendall(b'S')
        try:
            with context.wrap_socket(connection, server_side=True) as secure:
                size = struct.unpack('!I', receive(secure, 4))[0]
                assert 8 <= size <= 10000, 'invalid StartupMessage length'
                startup = receive(secure, size - 4)
                assert startup[:4] == struct.pack('!I', 196608), 'invalid protocol version'
                assert b'user\x00tls_smoke\x00' in startup, 'missing startup user'
                return True
        except ConnectionResetError:
            return False
        except ssl.SSLError as error:
            # SQLx/rustls may drop the socket without flushing its TLS alert.
            # Positive cases using this same peer must reach StartupMessage;
            # protocol/cipher failures must not masquerade as negative coverage.
            assert error.reason in ('TLSV1_ALERT_UNKNOWN_CA', 'SSLV3_ALERT_BAD_CERTIFICATE',
                                    'SSLV3_ALERT_CERTIFICATE_UNKNOWN',
                                    'UNEXPECTED_EOF_WHILE_READING'), error
            return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--trust', choices=['pem', 'untrusted', 'native'], default='pem')
    parser.add_argument('--cargo-config')
    args = parser.parse_args()
    cargo = ['cargo', 'test', '-p', 'uroborosql-lint', '--features', 'postgres-catalog',
             '--test', 'tls_smoke']
    if args.cargo_config:
        cargo += ['--config', str(Path(args.cargo_config).resolve())]
    else:
        cargo += ['--locked']
    subprocess.run(cargo + ['--no-run'], cwd=ROOT, check=True)
    environment = {key: value for key, value in os.environ.items()
                   if not key.upper().startswith(('PG', 'CATALOG_TEST_', 'SSL_CERT_'))}
    # Production settings must override a hostile attempt to disable TLS.
    environment['PGSSLMODE'] = 'disable'
    if args.trust == 'pem':
        environment['PGSSLROOTCERT'] = str(CERTS / 'ca.pem')
    cases = [('localhost.pem', host, args.trust != 'untrusted')
             for host in ['localhost', '127.0.0.1']]
    cases += [('mismatch.pem', host, False) for host in ['localhost', '127.0.0.1']]
    cases += [(None, '127.0.0.1', False)]
    for certificate, host, expected in cases:
        with socket.socket() as listener, ThreadPoolExecutor(max_workers=1) as executor:
            listener.bind(('127.0.0.1', 0))
            listener.listen(1)
            listener.settimeout(30)
            observed = executor.submit(serve, listener, certificate)
            env = dict(environment, CATALOG_TEST_HOST=host,
                       CATALOG_TEST_PORT=str(listener.getsockname()[1]))
            subprocess.run(cargo + ['tls_peer_connection', '--', '--ignored', '--exact',
                                    '--nocapture'], cwd=ROOT, env=env, check=True, timeout=60)
            assert observed.result(timeout=15) == expected, f'{args.trust}: {certificate} {host}'
            print(f'TLS {args.trust}: {certificate or "unsupported"} {host}: '
                  f'{"verified StartupMessage" if expected else "rejected"}', flush=True)


if __name__ == '__main__':
    main()
