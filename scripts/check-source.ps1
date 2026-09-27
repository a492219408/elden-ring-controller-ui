# SPDX-License-Identifier: GPL-3.0-only
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
foreach ($Arguments in @(
    @((Join-Path $RepositoryRoot 'tools/source_policy.py'), 'check'),
    @((Join-Path $RepositoryRoot 'tools/test_source_policy.py'))
)) {
    if (Get-Command mise -ErrorAction SilentlyContinue) {
        $MiseArguments = @('exec', '--', 'python') + $Arguments
        & mise @MiseArguments
    } else { & python @Arguments }
    if ($LASTEXITCODE -ne 0) { throw "Source policy check failed: $LASTEXITCODE" }
}
