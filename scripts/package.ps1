[CmdletBinding()]
param(
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
& (Join-Path $PSScriptRoot 'check-source.ps1')
$Version = (Select-String -Path (Join-Path $RepositoryRoot 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"$').Matches.Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw 'Cannot read package version from Cargo.toml'
}
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw "build.ps1 failed with exit code $LASTEXITCODE"
    }
}

$DistributionRoot = [System.IO.Path]::GetFullPath((Join-Path $RepositoryRoot 'dist'))
$PackageRoot = [System.IO.Path]::GetFullPath((Join-Path $DistributionRoot "EldenRingControllerUI-$Version"))
if (-not $PackageRoot.StartsWith($DistributionRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to package outside $DistributionRoot"
}
if (Test-Path -LiteralPath $PackageRoot) {
    throw "Package already exists; use a new version or a fresh build directory: $PackageRoot"
}

$ArchivePath = "$PackageRoot.zip"
$ArchiveHashPath = "$ArchivePath.sha256"
foreach ($Target in @($ArchivePath, $ArchiveHashPath)) {
    if (Test-Path -LiteralPath $Target) { throw "Release artifact already exists: $Target" }
}
$Natives = New-Item -ItemType Directory -Force -Path (Join-Path $PackageRoot 'natives')
$ReleaseDirectory = Join-Path $RepositoryRoot 'target\x86_64-pc-windows-msvc\release'
Copy-Item -LiteralPath (Join-Path $ReleaseDirectory 'EldenRingControllerUI.dll') -Destination $Natives.FullName
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'EldenRingControllerUI.ini') -Destination $Natives.FullName
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\EldenRingControllerUI.me3') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\release\README.zh-CN.txt') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\release\README.txt') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'LICENSE') -Destination (Join-Path $PackageRoot 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'NOTICE') -Destination (Join-Path $PackageRoot 'NOTICE.txt')
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'THIRD_PARTY_NOTICES.md') -Destination (Join-Path $PackageRoot 'THIRD_PARTY_NOTICES.txt')
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'third_party\Rust-COPYRIGHT-library.html') -Destination $PackageRoot

$ForbiddenAssets = Get-ChildItem -LiteralPath $PackageRoot -File -Recurse | Where-Object {
    $_.Extension -in @('.gfx', '.tga', '.dds', '.tpf', '.dcx', '.fmg', '.bin')
}
if ($ForbiddenAssets -or (Test-Path -LiteralPath (Join-Path $PackageRoot 'assets'))) {
    throw 'Pure-DLL package must not contain any resource files or assets directory'
}

& (Join-Path $PSScriptRoot 'verify-package.ps1') -PackageDirectory $PackageRoot
& (Join-Path $PSScriptRoot 'check-source.ps1')
Compress-Archive -LiteralPath $PackageRoot -DestinationPath $ArchivePath -CompressionLevel Optimal
$ArchiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText(
    $ArchiveHashPath,
    "$ArchiveHash  $([System.IO.Path]::GetFileName($ArchivePath))`n",
    [System.Text.Encoding]::ASCII
)

Write-Host "Package directory: $PackageRoot"
Write-Host "Package archive:   $ArchivePath"
Write-Host "SHA-256 file:      $ArchiveHashPath"
