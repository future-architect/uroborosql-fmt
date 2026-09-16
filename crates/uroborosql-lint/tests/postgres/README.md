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
