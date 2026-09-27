[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Invoke-CheckedCargo {
    param([Parameter(Mandatory)][string[]] $Arguments)

    $MiseCommand = Get-Command mise -ErrorAction SilentlyContinue
    if ($null -ne $MiseCommand) {
        $MiseArguments = @('exec', '--', 'cargo') + $Arguments
        & mise @MiseArguments
    }
    else {
        & cargo @Arguments
    }
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

Push-Location $RepositoryRoot
try {
    Invoke-CheckedCargo @('fmt', '--all', '--', '--check')
    Invoke-CheckedCargo @(
        'clippy', '--locked', '--all-targets', '--target', 'x86_64-pc-windows-msvc',
        '--', '-D', 'warnings'
    )
    Invoke-CheckedCargo @('test', '--locked', '--target', 'x86_64-pc-windows-msvc')
    Invoke-CheckedCargo @(
        'build', '--locked', '--release', '--target', 'x86_64-pc-windows-msvc'
    )
}
finally {
    Pop-Location
}
