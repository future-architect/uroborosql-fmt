#!/usr/bin/env python3
"""Run catalog tests against disposable Docker Compose services."""
import argparse
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def command(args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def output(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--major', type=int, choices=range(14, 19), action='append')
    parser.add_argument('--cargo-config')
    args = parser.parse_args()
    cargo = ['cargo', 'test', '-p', 'uroborosql-lint', '--features', 'postgres-catalog',
             '--test', 'postgres']
    if args.cargo_config:
        cargo += ['--config', str(Path(args.cargo_config).resolve())]
    command(cargo + ['--no-run'], cwd=ROOT)
    for major in args.major or range(14, 19):
        service = 'pg' + str(major)
        password = secrets.token_hex(24)
        compose_env = dict(os.environ, CATALOG_TEST_PASSWORD=password)
        compose = ['docker', 'compose', '-f', str(HERE / 'compose.yaml'),
                   '-p', 'urobo-catalog-' + secrets.token_hex(6)]
        previous = set(output(['docker', 'image', 'ls', '-q', '--no-trunc']).splitlines())
        image = output(compose + ['config', '--images', service], env=compose_env)
        image_id = None
        try:
            command(compose + ['pull', service], env=compose_env)
            image_id = output(['docker', 'image', 'inspect', '--format', '{{.Id}}', image])
            print('architecture=' + output(['docker', 'image', 'inspect', '--format', '{{.Architecture}}', image]), flush=True)
            command(compose + ['up', '-d', '--wait', '--wait-timeout', '60', service], env=compose_env)
            port = output(compose + ['port', service, '5432'], env=compose_env).rsplit(':', 1)[1]
            with tempfile.TemporaryDirectory(prefix='urobo-catalog-') as directory:
                temp = Path(directory)

                def sql(text):
                    return output(compose + ['exec', '-T', service, 'psql', '-X', '-U', 'postgres',
                                             '-v', 'ON_ERROR_STOP=1', '-At'], input=text, env=compose_env)

                sql((HERE / 'fixture.sql').read_text() + "\nALTER ROLE catalog_reader PASSWORD '" + password + "';")
                print(f"{service}: {image}; server_version_num={sql('SHOW server_version_num;')}", flush=True)
                environment = {key: value for key, value in os.environ.items()
                               if not key.startswith(('PG', 'CATALOG_TEST_'))}
                environment.update(CATALOG_TEST_PORT=port, CATALOG_TEST_PASSWORD=password)

                def test(test_name, changes=None):
                    env = dict(environment)
                    if changes:
                        env.update(changes)
                    command(cargo + [test_name, '--', '--ignored', '--exact', '--nocapture'], cwd=ROOT, env=env)

                for test_name in ['definitions_and_visibility', 'refreshes_each_acquisition', 'failures_are_not_absence', 'concurrent_ddl_keeps_one_snapshot']:
                    test(test_name)
                test('environment_is_explicitly_controlled', dict(
                    CATALOG_TEST_ENV='fallback', PGOPTIONS='-c search_path=app,public', PGPASSWORD=password,
                    PGHOST='invalid.example', PGPORT='1', PGUSER='invalid', PGDATABASE='invalid', PGSSLMODE='require'))
                test('environment_is_explicitly_controlled', dict(
                    CATALOG_TEST_ENV='explicit', PGOPTIONS='-c search_path=app,public', PGPASSWORD='wrong-environment-password'))
                passfile = temp / 'pgpass'
                passfile.write_text('127.0.0.1:' + port + ':postgres:postgres:' + password + '\n')
                passfile.chmod(0o600)
                test('pgpass_is_not_used', {'PGPASSFILE': str(passfile)})
                if major == 18:
                    test('query_timeout_and_cancellation_close_connections')
                    test('total_deadline_limits_multiple_successful_queries')
                    test('catalog_permission_failure_is_unavailable')
                    test('tls_policy', {'CATALOG_TEST_TLS': 'failure', 'PGSSLMODE': 'disable'})

                    def certificate(san):
                        command(compose + ['exec', '-T', service, 'sh', '/catalog-tls.sh', san],
                                env=compose_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                        command(compose + ['cp', service + ':/tmp/catalog-tls/ca.pem', str(temp / 'cert.pem')], env=compose_env)
                        time.sleep(0.2)

                    certificate('DNS:localhost,IP:127.0.0.1')
                    for host in ['localhost', '127.0.0.1']:
                        test('tls_policy', {'CATALOG_TEST_TLS': 'success', 'CATALOG_TEST_HOST': host,
                                           'PGSSLROOTCERT': str(temp / 'cert.pem'), 'PGSSLMODE': 'disable'})
                    test('tls_policy', {'CATALOG_TEST_TLS': 'failure'})
                    certificate('DNS:other.invalid,IP:127.0.0.2')
                    for host in ['localhost', '127.0.0.1']:
                        test('tls_policy', {'CATALOG_TEST_TLS': 'failure', 'CATALOG_TEST_HOST': host,
                                           'PGSSLROOTCERT': str(temp / 'cert.pem')})
                    test('definitions_and_visibility')
        finally:
            command(compose + ['down', '--volumes'], env=compose_env)
            if image_id and image_id not in previous:
                subprocess.run(['docker', 'image', 'rm', image], check=False)


if __name__ == '__main__':
    main()
