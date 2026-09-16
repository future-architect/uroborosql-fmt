# PostgreSQL catalog fixtures

Run from the repository root with Python 3, Cargo and an existing
Docker engine with Docker Compose and Linux container support:

    python3 crates/uroborosql-lint/tests/postgres/run.py

Use `--major 18` for the TLS/fault suite or `--cargo-config PATH` for a local
parser override. The script runs one version at a time, binds a random
loopback port, generates isolated credentials, and removes its containers,
networks, volumes, temporary files and newly pulled unshared images on exit.
It never installs host PostgreSQL or changes the OS certificate store.

`compose.yaml` declares the PostgreSQL 14–18 services and pins their official
image indexes by digest. Docker selects the matching platform image.
`run.py` starts one service at a time with `docker compose up --wait`, runs
Cargo tests with isolated environments, and calls `docker compose down --volumes`.
It prints the image reference, server version and ordinary Cargo test results;
there is no custom JSON report. Redirect output to a file to keep a test log.
Update all five image references together when refreshing the test matrix.

The integration tests are ignored by ordinary Cargo test runs because they
require disposable database superuser access. **Do not run them against an
existing database.** Fault tests temporarily replace a catalog function or
change catalog privileges; concurrent-DDL tests use an advisory-lock wrapper
to stop acquisition after its snapshot is established. The entire container
is disposable even when a test fails.

`tls.sh` runs inside the database container and signs server certificates with a generated CA and passes that
CA through `PGSSLROOTCERT` only to child test processes. Environment precedence
and pgpass tests also use child-process environments, without mutating the
running test process's environment.

Provider code is enabled by `postgres-catalog`. CLI/LSP integration and SQLite
providers are separate work.

## CI coverage

The reusable `.github/workflows/test.yml`, called by the PR/build workflow,
runs feature-enabled unit/contract tests on native Linux x64,
macOS Intel/ARM64 and Windows x64/ARM64 runners, with feature-enabled Clippy
on Linux. One separate Ubuntu job runs `run.py` sequentially for 14–18,
including the PG18 TLS/fault suite, sharing one build across all five versions.
The ignored database tests are explicitly executed by this runner.

Windows runners do not need Docker or a PostgreSQL installation. Instead,
`tls_windows.ps1` runs `tls_smoke.py` against a loopback PostgreSQL SSLRequest
peer implemented with Python's standard `ssl` module. It first checks rejection
of the untrusted fixture CA, temporarily imports only that CA into the
disposable runner's CurrentUser Root store, and then checks native-root TLS
success for DNS/IP names, name mismatch rejection, and refusal of non-TLS
peers. It refuses preexisting fixture trust and removes its certificate in
`finally`. The script rejects execution outside GitHub-hosted Windows CI.
No local host trust store should be changed.

On other runners, or locally, run:

    python3 crates/uroborosql-lint/tests/postgres/tls_smoke.py
    python3 crates/uroborosql-lint/tests/postgres/tls_smoke.py --trust untrusted

The default uses `PGSSLROOTCERT` in child processes; `--cargo-config PATH`
supports local parser overrides. The peer must observe a decrypted PostgreSQL
StartupMessage in each successful TLS case. It then closes without authentication,
so the Provider returns a connection error in both positive and negative cases.
This proves TLS verification through the production SQLx connection path;
it does not prove Windows PostgreSQL authentication or catalog acquisition.
The full PostgreSQL acquisition suite runs separately on Ubuntu.

`tls-smoke/` contains **public test-only** certificates and a server private key,
generated with OpenSSL RSA-2048/SHA-256, valid from 2026-09-16 to 2036-09-13.
The CA private key was discarded. The two server certificates use the same key:
one has SAN `DNS:localhost,IP:127.0.0.1`, the other
`DNS:other.invalid,IP:127.0.0.2`. Regenerate the CA and both leaf certificates
together before expiry, using CA basicConstraints/keyCertSign and leaf
serverAuth/digitalSignature/keyEncipherment extensions. Never use this public
key or CA for a real service.

Adding these jobs is not evidence that Windows/macOS Intel execution succeeded;
that claim requires successful workflow results on the relevant runners.
