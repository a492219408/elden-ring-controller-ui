# SPDX-License-Identifier: GPL-3.0-only
[CmdletBinding()]
param([string] $OutputDirectory)
$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $RepositoryRoot 'dist' }
$Arguments = @((Join-Path $RepositoryRoot 'tools/source_policy.py'), 'pack', '--output', $OutputDirectory)
if (Get-Command mise -ErrorAction SilentlyContinue) {
    $MiseArguments = @('exec', '--', 'python') + $Arguments
    & mise @MiseArguments
} else { & python @Arguments }
if ($LASTEXITCODE -ne 0) { throw "Source packaging failed: $LASTEXITCODE" }
