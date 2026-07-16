$ErrorActionPreference = "Stop"
$Version = if ($env:DEXDECK_VERSION) { $env:DEXDECK_VERSION } else { "0.2.0" }
$Repository = if ($env:DEXDECK_REPOSITORY) { $env:DEXDECK_REPOSITORY } else { "drilonrecica/dexdeck" }
$InstallDir = if ($env:DEXDECK_INSTALL_DIR) { $env:DEXDECK_INSTALL_DIR } else { Join-Path $HOME ".cargo\bin" }
$Target = "x86_64-pc-windows-msvc"
$Archive = "dexdeck-$Version-$Target.zip"
$Base = "https://github.com/$Repository/releases/download/v$Version"
$Temporary = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())

try {
    New-Item -ItemType Directory -Path $Temporary | Out-Null
    Invoke-WebRequest -UseBasicParsing "$Base/$Archive" -OutFile (Join-Path $Temporary $Archive)
    Invoke-WebRequest -UseBasicParsing "$Base/$Archive.sha256" -OutFile (Join-Path $Temporary "$Archive.sha256")
    $Expected = ((Get-Content (Join-Path $Temporary "$Archive.sha256")) -split " ")[0].ToLowerInvariant()
    $Actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Temporary $Archive)).Hash.ToLowerInvariant()
    if ($Expected -ne $Actual) { throw "DexDeck archive checksum mismatch" }
    Expand-Archive (Join-Path $Temporary $Archive) -DestinationPath $Temporary
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $Temporary "dexdeck-$Version-$Target\dexdeck.exe") (Join-Path $InstallDir "dexdeck.exe")
    Write-Host "DexDeck $Version installed at $InstallDir\dexdeck.exe"
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temporary
}
