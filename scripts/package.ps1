# Builds Bloop and its installer, and puts dist\Bloop-Setup-<version>.exe
# next to the repo root. Run from anywhere:  .\scripts\package.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    Write-Host 'Building bloop.exe (release)...'
    cargo build --release -p bloop
    if ($LASTEXITCODE -ne 0) { throw 'bloop build failed' }

    Write-Host 'Building the installer...'
    $env:BLOOP_PAYLOAD = Join-Path $root 'target\release\bloop.exe'
    cargo build --release -p bloop-setup
    if ($LASTEXITCODE -ne 0) { throw 'installer build failed' }

    $version = (Select-String -Path (Join-Path $root 'installer\Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force $dist | Out-Null
    $out = Join-Path $dist "Bloop-Setup-$version.exe"
    Copy-Item (Join-Path $root 'target\release\Bloop-Setup.exe') $out -Force
    $mb = [math]::Round((Get-Item $out).Length / 1MB, 1)
    Write-Host "Done: $out ($mb MB)"
}
finally {
    Remove-Item Env:\BLOOP_PAYLOAD -ErrorAction SilentlyContinue
    Pop-Location
}
