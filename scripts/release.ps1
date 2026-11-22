# Publishes a Bloop release on GitHub, which installed copies pick up as
# an automatic update within a day (or at once via "Check for updates").
#
#   .\scripts\release.ps1                 # release the version in Cargo.toml
#   .\scripts\release.ps1 -Version 0.2.0  # bump Cargo.toml first, then release
#   .\scripts\release.ps1 -Notes "Faster zoom; fixed GIF timing"
#
# Needs the GitHub CLI (gh), signed in, and a clean, pushed working tree.
param(
    [string]$Version,
    [string]$Notes
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
    if (-not $gh) { $gh = 'C:\Program Files\GitHub CLI\gh.exe' }
    if (-not (Test-Path $gh)) { throw 'GitHub CLI (gh) not found.' }

    if (git status --porcelain) { throw 'Commit or stash your changes first.' }

    $manifest = Join-Path $root 'Cargo.toml'
    if ($Version) {
        $Version = $Version.TrimStart('v')
        $text = Get-Content $manifest -Raw
        $text = [regex]::Replace($text, '(?m)^version = "[^"]+"', "version = `"$Version`"", 1)
        Set-Content $manifest $text -NoNewline -Encoding utf8
        cargo update -w --offline | Out-Null
        git add Cargo.toml Cargo.lock
        git commit -m "Release $Version"
    }
    $Version = (Select-String -Path $manifest -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
    $tag = "v$Version"

    & $gh release view $tag 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) { throw "Release $tag already exists. Bump the version with -Version." }

    & (Join-Path $PSScriptRoot 'package.ps1')
    $setup = Join-Path $root "dist\Bloop-Setup-$Version.exe"

    git push
    if ($LASTEXITCODE -ne 0) { throw 'git push failed' }

    $notesArgs = if ($Notes) { @('--notes', $Notes) } else { @('--generate-notes') }
    & $gh release create $tag $setup "$setup.sha256" --title "Bloop $Version" @notesArgs
    if ($LASTEXITCODE -ne 0) { throw 'gh release create failed' }
    Write-Host "Published $tag."
}
finally {
    Pop-Location
}
