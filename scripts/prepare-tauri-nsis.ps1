$ErrorActionPreference = "Stop"

$nsisUrl = "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip"
$nsisSha1 = "EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D"
$pluginUrl = "https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll"
$pluginSha1 = "75197FEE3C6A814FE035788D1C34EAD39349B860"
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$toolsRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\.tauri"))
$destination = [System.IO.Path]::GetFullPath((Join-Path $toolsRoot "NSIS"))
$markerPath = Join-Path $destination ".focusflow-nsis.sha1"
$pluginPath = Join-Path $destination "Plugins\x86-unicode\additional\nsis_tauri_utils.dll"
$taskTemp = Join-Path ([System.IO.Path]::GetTempPath()) ("focusflow-nsis-" + [guid]::NewGuid().ToString("N"))

function Get-HashHex([string]$Path, [System.Security.Cryptography.HashAlgorithm]$Algorithm) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return ([System.BitConverter]::ToString($Algorithm.ComputeHash($stream))).Replace("-", "").ToUpperInvariant()
    }
    finally {
        $Algorithm.Dispose()
        $stream.Dispose()
    }
}

$requiredFiles = @(
    (Join-Path $destination "makensis.exe"),
    (Join-Path $destination "Bin\makensis.exe"),
    (Join-Path $destination "Include\MUI2.nsh"),
    (Join-Path $destination "Plugins\x86-unicode\System.dll"),
    $pluginPath,
    $markerPath
)
$prepared = ($requiredFiles | Where-Object { -not (Test-Path -LiteralPath $_) }).Count -eq 0
if ($prepared) {
    $markerMatches = [System.IO.File]::ReadAllText($markerPath).Trim() -eq $nsisSha1
    $pluginHash = Get-HashHex $pluginPath ([System.Security.Cryptography.SHA1]::Create())
    if ($markerMatches -and $pluginHash -eq $pluginSha1) {
        Write-Host "Verified Tauri NSIS tools are already prepared."
        exit 0
    }
}

if (-not $destination.StartsWith($toolsRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to modify an NSIS cache outside the repository target directory."
}

try {
    New-Item -ItemType Directory -Path $taskTemp -Force | Out-Null
    $archivePath = Join-Path $taskTemp "nsis.zip"
    $extractPath = Join-Path $taskTemp "extracted"
    $downloadedPlugin = Join-Path $taskTemp "nsis_tauri_utils.dll"

    Write-Host "Downloading pinned Tauri NSIS 3.11 tools..."
    Invoke-WebRequest -Uri $nsisUrl -OutFile $archivePath -UseBasicParsing
    $archiveHash = Get-HashHex $archivePath ([System.Security.Cryptography.SHA1]::Create())
    if ($archiveHash -ne $nsisSha1) {
        throw "Tauri NSIS checksum mismatch. Expected $nsisSha1 but received $archiveHash."
    }

    Invoke-WebRequest -Uri $pluginUrl -OutFile $downloadedPlugin -UseBasicParsing
    $downloadedPluginHash = Get-HashHex $downloadedPlugin ([System.Security.Cryptography.SHA1]::Create())
    if ($downloadedPluginHash -ne $pluginSha1) {
        throw "Tauri NSIS plugin checksum mismatch. Expected $pluginSha1 but received $downloadedPluginHash."
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath -Force
    $verifiedSource = Join-Path $extractPath "nsis-3.11"
    if (-not (Test-Path -LiteralPath (Join-Path $verifiedSource "makensis.exe"))) {
        throw "The verified Tauri NSIS archive has an unexpected layout."
    }

    New-Item -ItemType Directory -Path $toolsRoot -Force | Out-Null
    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    Copy-Item -LiteralPath $verifiedSource -Destination $destination -Recurse
    New-Item -ItemType Directory -Path (Split-Path $pluginPath -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $downloadedPlugin -Destination $pluginPath -Force
    [System.IO.File]::WriteAllText($markerPath, $nsisSha1 + [Environment]::NewLine)
    Write-Host "Tauri NSIS tools are verified and ready."
}
finally {
    if (Test-Path -LiteralPath $taskTemp) {
        Remove-Item -LiteralPath $taskTemp -Recurse -Force
    }
}
