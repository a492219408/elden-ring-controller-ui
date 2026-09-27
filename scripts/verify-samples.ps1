[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $EldenRingExe,
    [Parameter(Mandatory)][string] $OfficialOptionsGfx,
    [Parameter(Mandatory)][string] $CommonOptionsGfx,
    [Parameter(Mandatory)][string] $KeyConfigurationGfx,
    [string] $ExportDirectory
)
$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ExePath = (Resolve-Path -LiteralPath $EldenRingExe).Path
$ExeLength = (Get-Item -LiteralPath $ExePath).Length
$ExeHash = (Get-FileHash -LiteralPath $ExePath -Algorithm SHA256).Hash
$SupportedExe = @(
    @(87024720, 'D1A84083C6C7C7902162FF098F7D86812839AA6B3575959398857E539C488134'),
    @(87042128, '1A3547101327F65D0C76DA2F9190AC0AA66871EA42BAE2AECC61E11A8B597891')
)
if (-not ($SupportedExe | Where-Object { $_[0] -eq $ExeLength -and $_[1] -eq $ExeHash })) {
    throw "Unexpected executable: $ExePath"
}
$Samples = @(
    @('ERCUI_ELDENRING_EXE', $ExePath, $ExeLength, $ExeHash),
    @('ERCUI_OFFICIAL_GFX', $OfficialOptionsGfx, 44007, '170996C2376BB14675FE1BB308C3CE82C28BEC0DEAFBFC8E40BD8FEF0E99E4B4'),
    @('ERCUI_COMMON_GFX', $CommonOptionsGfx, 46401, '4F80A029D8D6BDB7C6C893F13704E44C592C670C5ADFC8BA6B11C793FB5E0392'),
    @('ERCUI_KEY_GFX', $KeyConfigurationGfx, 55592, '693D6B509C01A4758BE63A17C55D043C0133FA2684B3A53CF5F21875202FF137')
)
$Previous = @{}
Push-Location $RepositoryRoot
try {
    foreach ($Sample in $Samples) {
        $Path = (Resolve-Path -LiteralPath $Sample[1]).Path
        if ((Get-Item -LiteralPath $Path).Length -ne $Sample[2] -or (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ne $Sample[3]) { throw "Unexpected sample: $Path" }
        $Previous[$Sample[0]] = [Environment]::GetEnvironmentVariable($Sample[0], 'Process')
        [Environment]::SetEnvironmentVariable($Sample[0], $Path, 'Process')
    }
    $Previous['ERCUI_EXPORT_GFX'] = [Environment]::GetEnvironmentVariable('ERCUI_EXPORT_GFX', 'Process')
    if ($ExportDirectory) {
        $Export = (Resolve-Path -LiteralPath $ExportDirectory).Path
        if (-not (Test-Path -LiteralPath $Export -PathType Container)) { throw 'Export directory must already exist' }
        [Environment]::SetEnvironmentVariable('ERCUI_EXPORT_GFX', $Export, 'Process')
    } else { Remove-Item -LiteralPath 'Env:ERCUI_EXPORT_GFX' -ErrorAction SilentlyContinue }
    $Checks = @(
        @('clippy','--locked','--all-targets','--features','game-fixtures','--target','x86_64-pc-windows-msvc','--','-D','warnings'),
        @('test','--locked','--features','game-fixtures','--target','x86_64-pc-windows-msvc','--','--include-ignored','--nocapture')
    )
    foreach ($Check in $Checks) {
        $MiseArguments = @('exec','--','cargo') + $Check
        if (Get-Command mise -ErrorAction SilentlyContinue) { & mise @MiseArguments }
        else { & cargo @Check }
        if ($LASTEXITCODE -ne 0) { throw "real-sample check failed: $LASTEXITCODE" }
    }
} finally {
    foreach ($Name in $Previous.Keys) {
        if ($null -eq $Previous[$Name]) { Remove-Item -LiteralPath "Env:$Name" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($Name, $Previous[$Name], 'Process') }
    }
    Pop-Location
}
Write-Host 'Known 1.17/1.17.1 executable and three official GFX samples passed; no game was launched.'
