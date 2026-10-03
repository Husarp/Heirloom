# Builds build\HeirloomSetup-<version>.exe (APP-STANDARDS.md section 1) and build\dist\Heirloom (the program folder).
#   1. the versions agree: Cargo.toml (the one place), package.json, src-tauri\tauri.conf.json
#   2. heirloom.exe: npx tauri build --no-bundle (the interface is built into the exe)
#   3. self-test: heirloom.exe --selftest starts hidden and checks the interface, the bundled fonts and an archive
#      round trip in a throwaway folder (skip with -NoSelfTest)
#   4. HeirloomSetup-<version>.exe: installer\setup.py with the program folder zipped inside it (PyInstaller)
#   5. build\BUILT.json: which version this run built
# No admin needed, at build time or install time. Run from any folder:
#   & "<project folder>\scripts\build.ps1" [-Python <python.exe with PyInstaller>] [-NoSelfTest]
param([switch]$NoSelfTest, [string]$Python = "")
$ErrorActionPreference = "Continue"   # (the tools write progress to stderr; failures are checked below)
$Root = Split-Path -Parent $PSScriptRoot
$Build = "$Root\build"
Set-Location $Root

if (-not $Python) { $Python = "$Root\.venv\Scripts\python.exe" }
if (-not (Test-Path $Python)) {
    throw "No Python at $Python - make one with: python -m venv .venv; .venv\Scripts\pip install pyinstaller (or pass -Python <path>)"
}
& $Python -m PyInstaller --version *> $null
if ($LASTEXITCODE) { throw "PyInstaller is not installed for $Python (pip install pyinstaller)" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" }

# 1. version agreement: an installer that says one version while the app says another is the drift this checks.
$version = (Select-String -Path "$Root\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$npmVersion = (Get-Content "$Root\package.json" -Raw | ConvertFrom-Json).version
$tauriVersion = (Get-Content "$Root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
if ($npmVersion -ne $version -or $tauriVersion -ne $version) {
    throw "Version mismatch: Cargo.toml says $version, package.json $npmVersion, tauri.conf.json $tauriVersion"
}
Write-Output "Building Heirloom $version"

# The stamp goes first: a build that fails halfway must not leave last run's version next to half-replaced files.
Remove-Item "$Build\BUILT.json" -ErrorAction SilentlyContinue
Remove-Item "$Build\HeirloomSetup-*.exe" -ErrorAction SilentlyContinue

# 2. the app (tauri runs "npm run build" first: type check and the interface)
& npx tauri build --no-bundle
if ($LASTEXITCODE) { throw "Building heirloom.exe failed" }
$exe = "$Root\target\release\heirloom.exe"
if (-not (Test-Path $exe)) { throw "No $exe after the build" }
# (Every file step stops the build: a failed copy must not leave last run's exe to be tested and packaged.)
$dist = "$Build\dist\Heirloom"
if (Test-Path $dist) { Remove-Item $dist -Recurse -Force -ErrorAction Stop }
New-Item -ItemType Directory -Force $dist -ErrorAction Stop | Out-Null
Copy-Item $exe $dist -ErrorAction Stop
if ((Get-FileHash $exe).Hash -ne (Get-FileHash "$dist\heirloom.exe").Hash) { throw "The program folder does not hold the exe just built" }

# 3. self-test, of the very exe that goes into the installer
if (-not $NoSelfTest) {
    $report = "$Build\selftest.txt"
    Remove-Item $report -ErrorAction SilentlyContinue
    $app = Start-Process "$dist\heirloom.exe" -ArgumentList "--selftest", "`"$report`"" -PassThru
    # The app's own watchdog answers within 60 s; one that never got that far (a system error box on a build server
    # with nobody to click it) would otherwise keep the build waiting for ever.
    if (-not $app.WaitForExit(120000)) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        throw "Self-test: heirloom.exe gave no answer within 2 minutes"
    }
    $result = Get-Content $report -Raw -Encoding UTF8 -ErrorAction SilentlyContinue
    if (-not $result -or -not $result.StartsWith("OK $version")) { throw "Self-test failed:`n$result" }
    Write-Output "Self-test: $($result.Trim())"
}

# 4. the installer, with the program folder zipped inside it and the version written for it
$zip = "$Build\payload.zip"
# Right after the self-test heirloom.exe can stay locked for a moment (WebView2 closing, antivirus), and
# Compress-Archive needs each file to itself: try again for a few seconds.
for ($try = 1; ; $try++) {
    Remove-Item $zip -ErrorAction SilentlyContinue
    try { Compress-Archive -Path "$dist\*" -DestinationPath $zip -CompressionLevel Optimal -ErrorAction Stop; break }
    catch { if ($try -ge 10) { throw }; Start-Sleep -Seconds 1 }
}
$gen = "$Build\installer"
New-Item -ItemType Directory -Force $gen -ErrorAction Stop | Out-Null
try { [System.IO.File]::WriteAllText("$gen\version.py", "VERSION = `"$version`"`n") } catch { throw "Could not write $gen\version.py: $_" }
Copy-Item "$Root\src-tauri\icons\icon.ico" "$gen\heirloom.ico" -Force -ErrorAction Stop
$name = "HeirloomSetup-$version"
& $Python -m PyInstaller --noconfirm --log-level WARN --onefile --noconsole --name $name `
    --icon "$gen\heirloom.ico" --paths $gen `
    --add-data "$zip;." --add-data "$gen\heirloom.ico;." `
    --distpath $Build --workpath "$Build\work-setup" --specpath "$Build\work-setup" installer\setup.py
if ($LASTEXITCODE) { throw "Building the installer failed" }
$setup = "$Build\$name.exe"
if (-not (Test-Path $setup)) { throw "No $setup after the build" }
$size = "{0:N1}" -f ((Get-Item $setup).Length / 1MB)
Write-Output "Built $setup ($size MB)"

# 5. build stamp, last on purpose: every failure above is a throw, so reaching this line means the build succeeded.
$built = [ordered]@{
    version   = $version
    builtAt   = Get-Date -Format "yyyy-MM-ddTHH:mm:ss"
    artifacts = @([ordered]@{ name = "$name.exe"; kind = "Windows" })
}
# WriteAllText, not Out-File: .NET writes UTF-8 with no BOM, which strict JSON readers need.
try { [System.IO.File]::WriteAllText("$Build\BUILT.json", ($built | ConvertTo-Json -Depth 3)) } catch { throw "Could not write BUILT.json: $_" }
Write-Output ""
Write-Output "BUILT $version - $name.exe"
