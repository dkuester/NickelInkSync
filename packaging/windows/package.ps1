# Builds the Windows release files into dist\:
#   NickelInkSync-<version>-windows-x64.zip       the desktop client
#                                                 (NickelInkSync.exe, with
#                                                 kepubify.exe next to it)
#   nickelinksync-cli-<version>-windows-x64.zip   the CLI
# Both with the licences. Needs Rust with the MSVC toolchain (the resource
# compiler puts the icon into the exe). Not code-signed: SmartScreen warns
# on first start (see the README).
#
# Windows does not tell upper from lower case in file names: the app and
# the CLI must differ by more than that (NickelInkSync vs nickelinksync),
# or one overwrites the other.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..\..')
$root = Get-Location

function Version($file) {
  (Select-String -Path $file -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
}
$version = Version 'Cargo.toml'
$guiVersion = Version 'gui\Cargo.toml'
if ($version -ne $guiVersion) {
  throw "Cargo.toml ($version) and gui\Cargo.toml ($guiVersion) differ"
}

# kepubify (MIT), a pinned release checked against this checksum.
$kepubifyVersion = 'v4.0.4'
$kepubifySha256 = 'f3dee3ece063329661fef474262fe403812335bf1676eab35b672565bf195738'

$target = 'x86_64-pc-windows-msvc'
$work = Join-Path $root 'target\package-windows'
$dist = Join-Path $root 'dist'
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force -Path $work, $dist | Out-Null

cargo build --release --locked --target $target -p nickelinksync -p nickelinksync-gui
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
$bin = Join-Path $root "target\$target\release"

# The PE subsystem of an exe: 2 a window program, 3 a console program.
function Subsystem($exe) {
  $bytes = [System.IO.File]::ReadAllBytes($exe)
  $pe = [BitConverter]::ToInt32($bytes, 0x3c)
  [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
}
$gui = Join-Path $bin 'nickelinksync-gui.exe'
if ((Subsystem $gui) -ne 2) { throw "$gui is not a window program: it would open a console" }

$kepubify = Join-Path $work 'kepubify.exe'
Invoke-WebRequest -UseBasicParsing -OutFile $kepubify `
  -Uri "https://github.com/pgaskin/kepubify/releases/download/$kepubifyVersion/kepubify-windows-64bit.exe"
$hash = (Get-FileHash -Algorithm SHA256 $kepubify).Hash.ToLower()
if ($hash -ne $kepubifySha256) { throw "kepubify-windows-64bit.exe: checksum $hash differs from the pinned one" }

# The desktop client; the app looks for kepubify next to itself.
$app = Join-Path $work "NickelInkSync-$version-windows-x64"
New-Item -ItemType Directory -Force -Path (Join-Path $app 'licenses') | Out-Null
Copy-Item $gui (Join-Path $app 'NickelInkSync.exe')
Copy-Item $kepubify (Join-Path $app 'kepubify.exe')
Copy-Item 'LICENSE' (Join-Path $app 'LICENSE.txt')
Copy-Item 'packaging\licenses\kepubify-LICENSE.txt' (Join-Path $app 'licenses\')

# The CLI.
$cli = Join-Path $work "nickelinksync-cli-$version-windows-x64"
New-Item -ItemType Directory -Force -Path $cli | Out-Null
Copy-Item (Join-Path $bin 'nickelinksync.exe') $cli
Copy-Item 'LICENSE' (Join-Path $cli 'LICENSE.txt')

# Each package is still what it should be.
if ((Subsystem (Join-Path $app 'NickelInkSync.exe')) -ne 2) { throw 'the app in the package is not the window program' }
if ((Subsystem (Join-Path $cli 'nickelinksync.exe')) -ne 3) { throw 'the CLI in the package is not the console program' }

foreach ($dir in $app, $cli) {
  $zip = Join-Path $dist ((Split-Path $dir -Leaf) + '.zip')
  if (Test-Path $zip) { Remove-Item $zip }
  Compress-Archive -Path $dir -DestinationPath $zip
  Write-Output $zip
}
