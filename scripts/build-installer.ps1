[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$IsccPath,
    [string]$CrtDir,
    [switch]$BootstrapCompiler
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$installerWork = Join-Path $repoRoot 'artifacts\installer'
$outputDir = Join-Path $repoRoot 'dist'
New-Item -ItemType Directory -Force -Path $installerWork, $outputDir | Out-Null

function Assert-SignedDownload([string]$Path, [string]$PublisherPattern) {
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch $PublisherPattern) {
        throw "Signature verification failed for $Path ($($signature.Status))."
    }
}

if (-not $IsccPath) {
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $IsccPath = $compiler.Source }
    foreach ($candidate in @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 7\ISCC.exe",
        (Join-Path $installerWork 'inno\ISCC.exe')
    )) {
        if (-not $IsccPath -and (Test-Path -LiteralPath $candidate)) { $IsccPath = $candidate }
    }
}

if (-not $IsccPath -and $BootstrapCompiler) {
    $compilerSetup = Join-Path $installerWork 'innosetup-6.7.3.exe'
    $compilerDir = Join-Path $installerWork 'inno'
    Invoke-WebRequest 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' -OutFile $compilerSetup
    Assert-SignedDownload $compilerSetup 'O=("?Pyrsys B\.V\.?"?|JRSoftware)(,|$)'
    $setup = Start-Process -FilePath $compilerSetup -ArgumentList @(
        '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER',
        '/NOICONS', '/TASKS=', ('/DIR="' + $compilerDir + '"')
    ) -WindowStyle Hidden -PassThru -Wait
    if ($setup.ExitCode -ne 0) { throw "Inno Setup installation failed: $($setup.ExitCode)" }
    $IsccPath = Join-Path $compilerDir 'ISCC.exe'
}

if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath)) {
    throw 'Install Inno Setup 6.3+ or use -BootstrapCompiler / -IsccPath.'
}

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        cargo build --release --workspace --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    $metadata = cargo metadata --format-version 1 --no-deps --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo metadata.' }
    $version = ($metadata.packages | Where-Object name -eq 'inna-app').version
    $binaryDir = Join-Path $metadata.target_directory 'release'
    foreach ($binary in @('anni.exe', 'inna-auth.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $binaryDir $binary))) {
            throw "Missing release binary: $binary"
        }
    }

    # App-local CRT DLLs keep setup per-user and avoid an elevated VC++ installer.
    # Use the redistributable files from Visual Studio, never system DLL copies.
    if (-not $CrtDir) {
        $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
        if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio not found. Pass -CrtDir with its x64 Microsoft.VC*.CRT directory.' }
        $visualStudioDir = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        $redistVersions = Get-ChildItem -LiteralPath (Join-Path $visualStudioDir 'VC\Redist\MSVC') -Directory |
            Where-Object Name -Match '^\d+\.\d+\.\d+$' | Sort-Object { [version]$_.Name } -Descending
        foreach ($redistVersion in $redistVersions) {
            $crtCandidate = Get-ChildItem -LiteralPath (Join-Path $redistVersion.FullName 'x64') -Directory -Filter 'Microsoft.VC*.CRT' |
                Select-Object -First 1
            if ($crtCandidate) { $CrtDir = $crtCandidate.FullName; break }
        }
        if (-not $CrtDir) { throw 'No x64 Visual C++ runtime found. Pass -CrtDir explicitly.' }
    }
    $runtimeDir = Join-Path $installerWork 'crt'
    New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null
    foreach ($dll in @('vcruntime140.dll', 'vcruntime140_1.dll')) {
        $runtimeSource = Join-Path $CrtDir $dll
        Assert-SignedDownload $runtimeSource 'O=Microsoft Corporation(,|$)'
        Copy-Item -LiteralPath $runtimeSource -Destination (Join-Path $runtimeDir $dll) -Force
    }

    # Microsoft documents packaging this signed Evergreen bootstrapper with apps.
    # It downloads the runtime only on machines where WebView2 is missing.
    $bootstrapper = Join-Path $installerWork 'MicrosoftEdgeWebview2Setup.exe'
    Invoke-WebRequest 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $bootstrapper
    Assert-SignedDownload $bootstrapper 'O=Microsoft Corporation(,|$)'

    & $IsccPath "/DAppVersion=$version" "/DSourceDir=$binaryDir" "/DOutputDir=$outputDir" "/DBootstrapperPath=$bootstrapper" "/DRuntimeDir=$runtimeDir" (Join-Path $repoRoot 'installer\anni.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    $installer = Join-Path $outputDir "Anni-$version-windows-x64-setup.exe"
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $(Split-Path -Leaf $installer)" | Set-Content -LiteralPath "$installer.sha256" -Encoding ascii
    Write-Output "Installer: $installer"
} finally {
    Pop-Location
}
