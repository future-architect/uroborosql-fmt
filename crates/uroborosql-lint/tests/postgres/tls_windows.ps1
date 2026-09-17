# Only the disposable GitHub-hosted Windows runner may change its user trust store.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Run this script only on a disposable GitHub-hosted Windows runner.'
}

Write-Host "Checking rejection before fixture CA is trusted."
python "$PSScriptRoot/tls_smoke.py" --trust untrusted
if ($LASTEXITCODE -ne 0) { throw 'Untrusted TLS smoke failed.' }

$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new("$PSScriptRoot/tls-smoke/ca.pem")
$path = "Cert:\CurrentUser\Root\$($certificate.Thumbprint)"
if (Test-Path $path) { throw 'Fixture CA already trusted; refusing to modify existing certificate.' }
try {
    # Import-Certificate can wait for interactive root-store consent on CI.
    Write-Host "Adding fixture CA to CurrentUser Root with certutil."
    certutil -user -f -addstore Root "$PSScriptRoot/tls-smoke/ca.pem"
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $path)) { throw 'Fixture CA import failed.' }
    Write-Host "Checking TLS with Windows native roots."
    python "$PSScriptRoot/tls_smoke.py" --trust native
    if ($LASTEXITCODE -ne 0) { throw 'Native-root TLS smoke failed.' }
} finally {
    Write-Host "Removing fixture CA from CurrentUser Root."
    if (Test-Path $path) {
        certutil -user -delstore Root $certificate.Thumbprint
        if ($LASTEXITCODE -ne 0 -or (Test-Path $path)) { throw 'Fixture CA cleanup failed.' }
    }
    $certificate.Dispose()
}
