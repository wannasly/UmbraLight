$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$thirdParty = Join-Path $root 'target\third-party'
New-Item -ItemType Directory -Force -Path $thirdParty | Out-Null

function Get-VerifiedArchive([string]$url, [string]$path, [string]$sha256) {
    if (-not (Test-Path -LiteralPath $path)) {
        Invoke-WebRequest -Uri $url -OutFile $path
    }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($actual -ne $sha256) {
        throw "SHA-256 mismatch for $path. Expected $sha256, got $actual"
    }
}

Push-Location $root
try {
    cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) { throw 'Cargo release build failed.' }

    $singBoxZip = Join-Path $thirdParty 'sing-box-1.13.14-windows-amd64.zip'
    Get-VerifiedArchive `
        'https://github.com/SagerNet/sing-box/releases/download/v1.13.14/sing-box-1.13.14-windows-amd64.zip' `
        $singBoxZip `
        'F580782C6DD10F7691C66CEA1D7C421813C5FBF7E305D1EE7CE0C3A40D196341'
    Expand-Archive -LiteralPath $singBoxZip -DestinationPath (Join-Path $thirdParty 'sing-box-1.13.14') -Force

    $wintunZip = Join-Path $thirdParty 'wintun-0.14.1.zip'
    Get-VerifiedArchive `
        'https://www.wintun.net/builds/wintun-0.14.1.zip' `
        $wintunZip `
        '07C256185D6EE3652E09FA55C0B673E2624B565E02C4B9091C79CA7D2F24EF51'
    Expand-Archive -LiteralPath $wintunZip -DestinationPath (Join-Path $thirdParty 'wintun-0.14.1') -Force

    $compiler = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    if (-not $compiler) {
        throw 'Inno Setup 6 is required. Install it with: winget install --id JRSoftware.InnoSetup --exact'
    }
    & $compiler (Join-Path $root 'installer\UmbraLight.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup build failed.' }
    Write-Output (Join-Path $root 'target\installer\UmbraLight-1.0.0-windows-x64-setup.exe')
} finally {
    Pop-Location
}
