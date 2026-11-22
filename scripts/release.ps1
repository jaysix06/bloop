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
# Native tools (git, cargo, gh) write progress to stderr, which Windows
# PowerShell 5.1 would turn into errors under 'Stop'; exit codes are checked
# explicitly instead.
$ErrorActionPreference = 'Continue'
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
        # Only the first `version = ` line: [workspace.package]. (The static
        # [regex]::Replace's 4th argument is RegexOptions, not a count.)
        $re = New-Object regex '(?m)^version = "[^"]+"'
        $text = $re.Replace($text, "version = `"$Version`"", 1)
        # UTF-8 without a BOM (PowerShell 5.1's -Encoding utf8 adds one).
        [IO.File]::WriteAllText($manifest, $text, (New-Object Text.UTF8Encoding $false))
        cargo update -w --offline | Out-Null
        git add Cargo.toml Cargo.lock
        git commit -m "Release $Version"
    }
    $Version = (Select-String -Path $manifest -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
    $tag = "v$Version"

    & $gh release view $tag *> $null
    if ($LASTEXITCODE -eq 0) { throw "Release $tag already exists. Bump the version with -Version." }

    & (Join-Path $PSScriptRoot 'package.ps1')
    if (-not $?) { throw 'packaging failed' }
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
