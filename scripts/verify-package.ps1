# SPDX-License-Identifier: GPL-3.0-only
param(
    [Parameter(Mandatory)][string] $PackageDirectory,
    [string] $SourceArchive,
    [string] $ReportPath
)
$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Arguments = @((Join-Path $RepositoryRoot 'tools\verify-release.py'), '--package', $PackageDirectory)
if ($SourceArchive) { $Arguments += @('--source', $SourceArchive) }
if ($ReportPath) { $Arguments += @('--report', $ReportPath) }
if (Get-Command mise -ErrorAction SilentlyContinue) {
    $MiseArguments = @('exec','--','python') + $Arguments
    & mise @MiseArguments
} else { & python @Arguments }
if ($LASTEXITCODE -ne 0) { throw "Release verification failed: $LASTEXITCODE" }
