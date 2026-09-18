#!/usr/bin/env python3
"""Run catalog tests against disposable Docker Compose services."""
import argparse
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile

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
    parser.add_argument('--sqlite', action='store_true', help='also test SQLite export, provider parity and offline product CLI')
    parser.add_argument('--cli', action='store_true', help='also test the product CLI and public async API')
    parser.add_argument('--review-sql', type=Path, help='inspect a SQL file against the disposable fixture')
    args = parser.parse_args()
    cargo = ['cargo', 'test', '-p', 'uroborosql-lint', '--features', 'postgres-catalog',
             *(['--lib'] if args.review_sql else ['--test', 'postgres'])]
    if args.cargo_config:
        cargo += ['--config', str(Path(args.cargo_config).resolve())]
    command(cargo + ['--no-run'], cwd=ROOT)
    cli_cargo = ['cargo', 'test', '-p', 'uroborosql-lint-cli', '--test', 'postgres']
    if args.cargo_config:
        cli_cargo += ['--config', str(Path(args.cargo_config).resolve())]
    if args.cli:
        command(cli_cargo + ['--no-run'], cwd=ROOT)
    sqlite_cargo = ['cargo', 'test', '-p', 'uroborosql-lint', '--features', 'postgres-catalog,sqlite-catalog', '--test', 'sqlite_postgres']
    export_cargo = ['cargo', 'test', '-p', 'uroborosql-lint-cli', '--test', 'export']
    if args.cargo_config:
        for invocation in [sqlite_cargo, export_cargo]:
            invocation += ['--config', str(Path(args.cargo_config).resolve())]
    if args.sqlite:
        for invocation in [sqlite_cargo, export_cargo]:
            command(invocation + ['--no-run'], cwd=ROOT)
    for major in args.major or ([18] if args.review_sql else range(14, 19)):
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

                if args.review_sql:
                    test('linter::catalog_tests::inspect_postgres_sql', {
                        'CATALOG_REVIEW_SQL': str(args.review_sql.resolve()),
                    })
                    continue

                if args.sqlite:
                    for invocation in [sqlite_cargo, export_cargo]:
                        command(invocation + ['--', '--ignored', '--nocapture'], cwd=ROOT, env=environment)

                if args.cli:
                    command(cli_cargo + ['--', '--ignored', '--nocapture'], cwd=ROOT, env=environment)

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

                if args.sqlite and major == 18:
                    metadata = ['cargo', 'metadata', '--format-version', '1', '--no-deps']
                    if args.cargo_config:
                        metadata += ['--config', str(Path(args.cargo_config).resolve())]
                    target = Path(json.loads(output(metadata, cwd=ROOT))['target_directory'])
                    binary = target / 'debug' / ('uroborosql-lint.exe' if os.name == 'nt' else 'uroborosql-lint')
                    snapshot = temp / 'offline.sqlite'
                    command([str(binary), 'export-catalog', '--host', '127.0.0.1', '--port', port,
                             '--user', 'catalog_reader', '--dbname', 'postgres', '--tls-mode', 'disable',
                             '--output', str(snapshot)], env=dict(environment, PGPASSWORD=password))
                    (temp / 'query.sql').write_text('SELECT missing FROM public.users;')
                    (temp / 'lint.json').write_text('{"db":{"schemaProvider":"file","path":"offline.sqlite"}}')
                    command(compose + ['stop', service], env=compose_env)
                    offline_env = {key: value for key, value in environment.items()
                                   if not key.startswith(('PG', 'CATALOG_TEST_'))}
                    offline = subprocess.run([str(binary), 'query.sql', '--config', 'lint.json'],
                                             cwd=temp, env=offline_env, capture_output=True, text=True)
                    assert offline.returncode == 1, offline.stderr
                    assert 'query.sql:1:8: error: no-unknown-reference' in offline.stdout, offline.stdout
                    assert 'complete=1 excluded=0 failed=0' in offline.stderr, offline.stderr
                    print('offline CLI after PostgreSQL stop: ok', flush=True)
        finally:
            command(compose + ['down', '--volumes'], env=compose_env)
            if image_id and image_id not in previous:
                subprocess.run(['docker', 'image', 'rm', image], check=False)


if __name__ == '__main__':
    main()
