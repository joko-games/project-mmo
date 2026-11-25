# Builds the game in release, starts a local galaxy server if none is listening, then the game.
# The server stops when the game window closes.
#
#   ./run-release.ps1              build, then play
#   ./run-release.ps1 -SkipBuild   play the build already made
#   ./run-release.ps1 -Clients 2   two game windows on the same galaxy

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$skipBuild = $false
$clients = 1
for ($i = 0; $i -lt $args.Count; $i++) {
    switch -Exact ($args[$i]) {
        '-SkipBuild' { $skipBuild = $true }
        '-Clients'   { $i++; $clients = [int]$args[$i] }
        default      { throw "unknown option $($args[$i])" }
    }
}

$port = 7777
$serverExe = 'target/release/mmo-server.exe'
$clientExe = 'target/release/mmo-client.exe'
$log = 'target/server.log'
$errLog = 'target/server.err.log'

function Test-Listening {
    return [bool](Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue)
}

if (-not $skipBuild) {
    Write-Host '==> building in release (the first build takes several minutes)' -ForegroundColor Cyan
    cargo build --workspace --release
    if ($LASTEXITCODE -ne 0) { throw 'the build failed' }
}

$server = $null
$games = @()
try {
    if (Test-Listening) {
        Write-Host "==> a galaxy server already listens on $port, using it" -ForegroundColor Yellow
    }
    else {
        Write-Host "==> starting the galaxy server, logging to $log" -ForegroundColor Cyan
        $server = Start-Process -FilePath $serverExe -NoNewWindow -PassThru `
            -RedirectStandardOutput $log -RedirectStandardError $errLog
        $deadline = (Get-Date).AddSeconds(15)
        while (-not (Test-Listening)) {
            if ($server.HasExited) { throw "the server stopped at once, see $log and $errLog" }
            if ((Get-Date) -gt $deadline) { throw "the server never listened on $port, see $log" }
            Start-Sleep -Milliseconds 150
        }
    }
    Write-Host '==> starting the game' -ForegroundColor Cyan
    for ($n = 1; $n -le $clients; $n++) {
        $games += Start-Process -FilePath $clientExe -PassThru
    }
    $games | Wait-Process
}
finally {
    foreach ($g in $games) {
        if ($g -and -not $g.HasExited) { Stop-Process -Id $g.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($server -and -not $server.HasExited) {
        Write-Host '==> stopping the galaxy server' -ForegroundColor Cyan
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
}
