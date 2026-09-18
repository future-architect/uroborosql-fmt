# PostgreSQL catalog fixtures

Run from the repository root with Python 3, Cargo and an existing
Docker engine with Docker Compose and Linux container support:

    python3 crates/uroborosql-lint/tests/postgres/run.py

Use `--major 18` for the fault suite or `--cargo-config PATH` for a local
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

Environment precedence and pgpass tests use child-process environments,
without mutating the running test process's environment.

Provider code is enabled by `postgres-catalog`. CLI/LSP integration and SQLite
providers are separate work.

## CI coverage

The reusable `.github/workflows/test.yml`, called by the PR/build workflow,
runs feature-enabled unit/contract tests on native Linux x64,
macOS Intel/ARM64 and Windows x64/ARM64 runners, with feature-enabled Clippy
on Linux. One separate Ubuntu job runs `run.py` sequentially for 14–18,
including the PG18 fault suite, sharing one build across all five versions.
The ignored database tests are explicitly executed by this runner.

Windows and macOS runners execute feature-enabled unit/contract tests.
PostgreSQL acquisition and fault cases run separately on Ubuntu.
TLS uses SQLx's standard implementation; dedicated TLS fixtures and
certificate-store manipulation are outside this test suite's scope.
