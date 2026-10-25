[CmdletBinding()]
param([string]$Installer)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$testDir = Join-Path $repoRoot 'artifacts\installer-smoke'
$logDir = Join-Path $repoRoot 'artifacts\installer'
$registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F78D1490-8623-462E-A75C-F8E022A94284}_is1'
$shortcutDir = Join-Path ([Environment]::GetFolderPath('Programs')) 'Anni'
$startupKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
if (Test-Path -LiteralPath $registration) { throw 'Anni is already installed. Run this smoke test on a clean account.' }
if (Test-Path -LiteralPath $testDir) { throw "Smoke test directory already exists: $testDir" }
if (Test-Path -LiteralPath $shortcutDir) { throw 'Anni shortcuts already exist. Run this smoke test on a clean account.' }
if (Get-ItemProperty -LiteralPath $startupKey -Name anni -ErrorAction SilentlyContinue) { throw 'Anni already has a startup entry. Run this smoke test on a clean account.' }
if (-not $Installer) {
    $Installer = (Get-ChildItem (Join-Path $repoRoot 'dist') -Filter '*-setup.exe' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Installer) { throw 'Build the installer first.' }
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Invoke-SilentSetup([string]$Executable, [string[]]$SetupArguments) {
    $process = Start-Process -FilePath $Executable -ArgumentList $SetupArguments -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Setup failed with exit code $($process.ExitCode): $Executable" }
}

$uninstaller = Join-Path $testDir 'unins000.exe'
try {
    foreach ($pass in @('install', 'upgrade')) {
        $modeArgument = if ($pass -eq 'install') { '/TASKS=startup' } else { '/AUTOMATICUPDATE=1' }
        Invoke-SilentSetup $Installer @(
            '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', $modeArgument,
            ('/DIR="' + $testDir + '"'),
            ('/LOG="' + (Join-Path $logDir "$pass.log") + '"')
        )
        foreach ($file in @('anni.exe', 'inna-auth.exe', 'vcruntime140.dll', 'vcruntime140_1.dll', 'unins000.exe', 'browser-extension\manifest.json', 'browser-extension\background.mjs', 'browser-extension\bridge.mjs', 'browser-extension\pair.js', 'browser-extension\popup.html', 'browser-extension\popup.js', 'browser-extension\popup.css')) {
            if (-not (Test-Path -LiteralPath (Join-Path $testDir $file))) { throw "Missing installed file: $file" }
        }
        $installed = Get-ItemProperty -LiteralPath $registration
        if ($installed.InstallLocation.TrimEnd('\') -ne $testDir) { throw 'Incorrect uninstall registration.' }
        $startup = Get-ItemProperty -LiteralPath $startupKey -Name anni -ErrorAction SilentlyContinue
        if ($pass -eq 'install') {
            $expected = '"' + (Join-Path $testDir 'anni.exe') + '" --desktop'
            if (-not $startup -or $startup.anni -ne $expected) { throw 'Setup did not register Anni startup.' }
            # Simulate disabling startup in Anni. A silent update must honor it.
            Remove-ItemProperty -LiteralPath $startupKey -Name anni
        } elseif ($startup) { throw 'An automatic update re-enabled a disabled startup preference.' }
    }
    foreach ($binary in @('anni', 'inna-auth')) {
        $helpOutput = Join-Path $logDir "$binary-help.txt"
        $app = Start-Process -FilePath (Join-Path $testDir "$binary.exe") -ArgumentList '--help' -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput $helpOutput
        if ($app.ExitCode -ne 0 -or (Get-Content -LiteralPath $helpOutput -Raw) -notmatch 'Usage:') {
            throw "Installed $binary did not start correctly."
        }
    }
    foreach ($shortcut in @('Anni.lnk', 'Anni desktop widgets.lnk')) {
        if (-not (Test-Path -LiteralPath (Join-Path $shortcutDir $shortcut))) { throw "Missing shortcut: $shortcut" }
    }
    # Also verify that uninstall removes an enabled startup entry belonging to this install.
    Set-ItemProperty -LiteralPath $startupKey -Name anni -Value ('"' + (Join-Path $testDir 'anni.exe') + '" --desktop')
} finally {
    if (Test-Path -LiteralPath $uninstaller) {
        Invoke-SilentSetup $uninstaller @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', ('/LOG="' + (Join-Path $logDir 'uninstall.log') + '"'))
    }
}
if (Test-Path -LiteralPath (Join-Path $testDir 'anni.exe')) { throw 'Uninstall left the app executable behind.' }
if (Test-Path -LiteralPath $registration) { throw 'Uninstall left the app registration behind.' }
if (Get-ItemProperty -LiteralPath $startupKey -Name anni -ErrorAction SilentlyContinue) { throw 'Uninstall left the app startup entry behind.' }
Write-Output 'Installer smoke test passed: install, automatic upgrade, startup opt-out preservation, executable startup, shortcuts, and uninstall.'
