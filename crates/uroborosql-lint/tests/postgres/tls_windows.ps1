# Only the disposable GitHub-hosted Windows runner may change its user trust store.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Run this script only on a disposable GitHub-hosted Windows runner.'
}

python "$PSScriptRoot/tls_smoke.py" --trust untrusted
if ($LASTEXITCODE -ne 0) { throw 'Untrusted TLS smoke failed.' }

$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new("$PSScriptRoot/tls-smoke/ca.pem")
$path = "Cert:\CurrentUser\Root\$($certificate.Thumbprint)"
if (Test-Path $path) { throw 'Fixture CA already trusted; refusing to modify existing certificate.' }
try {
    Import-Certificate -FilePath "$PSScriptRoot/tls-smoke/ca.pem" -CertStoreLocation Cert:\CurrentUser\Root | Out-Null
    python "$PSScriptRoot/tls_smoke.py" --trust native
    if ($LASTEXITCODE -ne 0) { throw 'Native-root TLS smoke failed.' }
} finally {
    if (Test-Path $path) { Remove-Item $path }
    $certificate.Dispose()
}
