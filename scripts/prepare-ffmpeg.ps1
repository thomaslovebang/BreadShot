param(
    [string]$DestinationRoot = (Join-Path $PSScriptRoot "..\apps\desktop\src-tauri")
)

$ErrorActionPreference = "Stop"
$packageUrl = "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip"
$expectedSha256 = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"
$taskTemp = Join-Path ([System.IO.Path]::GetTempPath()) ("focusflow-ffmpeg-" + [guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $taskTemp "ffmpeg.zip"
$extractPath = Join-Path $taskTemp "extracted"
$binDestination = Join-Path $DestinationRoot "bin"
$legalDestination = Join-Path $DestinationRoot "legal"
$markerPath = Join-Path $legalDestination "FFmpeg-package.sha256"

function Get-Sha256Hex([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = $sha256.ComputeHash($stream)
        return ([System.BitConverter]::ToString($bytes)).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $sha256.Dispose()
        $stream.Dispose()
    }
}

$preparedFiles = @(
    (Join-Path $binDestination "ffmpeg.exe"),
    (Join-Path $binDestination "ffprobe.exe"),
    (Join-Path $legalDestination "FFmpeg-GPL-3.0.txt"),
    (Join-Path $legalDestination "FFmpeg-build-README.txt"),
    $markerPath
)
$prepared = ($preparedFiles | Where-Object { -not (Test-Path -LiteralPath $_) }).Count -eq 0
if ($prepared -and ([System.IO.File]::ReadAllText($markerPath).Trim() -eq $expectedSha256)) {
    $encoderOutput = & (Join-Path $binDestination "ffmpeg.exe") -hide_banner -encoders 2>&1
    if ($encoderOutput | Select-String -SimpleMatch "libx264") {
        Write-Host "Verified FFmpeg bundle resources are already prepared."
        exit 0
    }
}

try {
    New-Item -ItemType Directory -Path $taskTemp, $extractPath -Force | Out-Null
    Write-Host "Downloading pinned FFmpeg 9.0.2 Essentials build..."
    Invoke-WebRequest -Uri $packageUrl -OutFile $archivePath -UseBasicParsing

    $actualSha256 = Get-Sha256Hex $archivePath
    if ($actualSha256 -ne $expectedSha256) {
        throw "FFmpeg package checksum mismatch. Expected $expectedSha256 but received $actualSha256."
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath -Force
    $ffmpeg = Get-ChildItem -LiteralPath $extractPath -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
    $ffprobe = Get-ChildItem -LiteralPath $extractPath -Recurse -Filter "ffprobe.exe" | Select-Object -First 1
    $license = Get-ChildItem -LiteralPath $extractPath -Recurse -File | Where-Object { $_.Name -eq "LICENSE" } | Select-Object -First 1
    $buildReadme = Get-ChildItem -LiteralPath $extractPath -Recurse -File | Where-Object { $_.Name -eq "README.txt" } | Select-Object -First 1

    if (-not $ffmpeg -or -not $ffprobe -or -not $license -or -not $buildReadme) {
        throw "The verified FFmpeg package is missing an expected binary or legal file."
    }

    New-Item -ItemType Directory -Path $binDestination, $legalDestination -Force | Out-Null
    Copy-Item -LiteralPath $ffmpeg.FullName -Destination (Join-Path $binDestination "ffmpeg.exe") -Force
    Copy-Item -LiteralPath $ffprobe.FullName -Destination (Join-Path $binDestination "ffprobe.exe") -Force
    Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $legalDestination "FFmpeg-GPL-3.0.txt") -Force
    Copy-Item -LiteralPath $buildReadme.FullName -Destination (Join-Path $legalDestination "FFmpeg-build-README.txt") -Force
    [System.IO.File]::WriteAllText($markerPath, $expectedSha256 + [Environment]::NewLine)

    $encoderOutput = & (Join-Path $binDestination "ffmpeg.exe") -hide_banner -encoders 2>&1
    if (-not ($encoderOutput | Select-String -SimpleMatch "libx264")) {
        throw "The prepared FFmpeg binary does not provide the required libx264 encoder."
    }

    Write-Host "FFmpeg is verified and ready for the FocusFlow bundle."
}
finally {
    if (Test-Path -LiteralPath $taskTemp) {
        Remove-Item -LiteralPath $taskTemp -Recurse -Force
    }
}
