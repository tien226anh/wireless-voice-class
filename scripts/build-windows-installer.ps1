param(
    [Parameter(Mandatory)]
    [ValidatePattern('\Av(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\z')]
    [string]$Version,
    [string]$PackageDir = "package",
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$package = (Resolve-Path $PackageDir).Path
$appVersion = $Version.Substring(1)
foreach ($file in @("wireless-pa.exe", "LICENSE", "VERSION.txt", "assets/icons/wireless-pa.ico", "docs/USER_GUIDE.md", "docs/USER_GUIDE_VI.md", "docs/FONT_LICENSE.txt")) {
    if (-not (Test-Path (Join-Path $package $file) -PathType Leaf)) {
        throw "Missing installer input: $file"
    }
}
$versionLine = (Get-Content (Join-Path $package "VERSION.txt"))[0]
if ($versionLine -ne "Wireless PA $Version") { throw "Staged package version does not match $Version" }
$binaryVersion = (Get-Item (Join-Path $package "wireless-pa.exe")).VersionInfo.ProductVersion
if ($binaryVersion -ne $appVersion) { throw "Executable version $binaryVersion does not match $appVersion" }

New-Item -ItemType Directory -Force $OutputDir | Out-Null
$output = (Resolve-Path $OutputDir).Path
$compiler = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6/ISCC.exe"
if (-not (Test-Path $compiler)) {
    $compiler = (Get-Command ISCC.exe -ErrorAction Stop).Source
}
$definition = Join-Path $PSScriptRoot "../packaging/windows/wireless-pa.iss"
& $compiler "/DAppVersion=$appVersion" "/DReleaseTag=$Version" "/DPackageDir=$package" "/DInstallerOutput=$output" $definition
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed with exit code $LASTEXITCODE" }
$installer = Join-Path $output "wireless-pa-$Version-windows-x64-setup.exe"
if ((Get-Item $installer).Length -eq 0) { throw "Installer is empty" }
Write-Host "Built installer: $installer"
