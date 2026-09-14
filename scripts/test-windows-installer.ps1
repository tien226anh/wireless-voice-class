param(
    [Parameter(Mandatory)]
    [ValidatePattern('\Av(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\z')]
    [string]$Version
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$registryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{5871AA42-5399-4C16-B6BF-8A946B6AA938}_is1"
if (Test-Path $registryPath) { throw "Run installer tests in a clean Windows account without Wireless PA installed" }
$installer = (Resolve-Path "dist/wireless-pa-$Version-windows-x64-setup.exe").Path
$package = (Resolve-Path "package").Path
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("wireless-pa-installer-" + [guid]::NewGuid())
$installDir = Join-Path $testRoot "Installed app with spaces"
$startShortcut = Join-Path ([Environment]::GetFolderPath("Programs")) "Wireless PA.lnk"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("DesktopDirectory")) "Wireless PA.lnk"
if ((Test-Path $startShortcut) -or (Test-Path $desktopShortcut)) { throw "Installer test shortcuts already exist" }
New-Item -ItemType Directory -Force $testRoot | Out-Null
$preferences = Join-Path $env:APPDATA "wireless-pa/data"
New-Item -ItemType Directory -Force $preferences | Out-Null
$preferenceMarker = Join-Path $preferences ("installer-test-" + [guid]::NewGuid() + ".txt")
Set-Content -LiteralPath $preferenceMarker -Value "Preserve user preferences"

function Invoke-Setup([string]$Executable, [string[]]$Arguments) {
    $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "Installer process failed: $Executable (exit $($process.ExitCode))" }
}

function Assert-Installed {
    Get-ChildItem $package -Recurse -File | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($package, $_.FullName)
        $installed = Join-Path $installDir $relative
        if (-not (Test-Path $installed)) { throw "Installer omitted $relative" }
        if ((Get-FileHash $_.FullName).Hash -ne (Get-FileHash $installed).Hash) { throw "Installed file differs: $relative" }
    }
    if ((Get-ItemProperty $registryPath).DisplayVersion -ne $Version.Substring(1)) { throw "Incorrect uninstall version" }
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($startShortcut)
    if ($shortcut.TargetPath -ne (Join-Path $installDir "wireless-pa.exe")) { throw "Incorrect Start menu shortcut" }
}

try {
    $common = @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-", "/DIR=`"$installDir`"")
    Invoke-Setup $installer ($common + @("/LOG=`"$testRoot/install.log`""))
    Assert-Installed
    if (Test-Path $desktopShortcut) { throw "Desktop shortcut must be opt-in" }

    # Reinstall into the same location; all staged files and one uninstall entry remain.
    Invoke-Setup $installer ($common + @("/TASKS=desktopicon", "/LOG=`"$testRoot/reinstall.log`""))
    Assert-Installed
    if (-not (Test-Path $desktopShortcut)) { throw "Requested desktop shortcut was not created" }
    if (-not (Test-Path $preferenceMarker)) { throw "Reinstall removed user preferences" }

    Invoke-Setup (Join-Path $installDir "unins000.exe") @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$testRoot/uninstall.log`"")
    foreach ($path in @((Join-Path $installDir "wireless-pa.exe"), $registryPath, $startShortcut, $desktopShortcut)) {
        if (Test-Path $path) { throw "Uninstall did not remove $path" }
    }
    if (-not (Test-Path $preferenceMarker)) { throw "Uninstall removed user preferences" }
    Write-Host "Installer passed install, payload hash, shortcuts, reinstall, uninstall, and preference-preservation checks."
} catch {
    Get-ChildItem $testRoot -Filter *.log | ForEach-Object { Get-Content $_.FullName -Tail 80 }
    throw
} finally {
    Remove-Item -LiteralPath $preferenceMarker -ErrorAction SilentlyContinue
}
