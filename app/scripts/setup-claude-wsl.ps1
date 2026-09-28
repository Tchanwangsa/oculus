# Sets up a dedicated Linux environment for the Windows Claude bridge.
# Run from PowerShell: .\app\scripts\setup-claude-wsl.ps1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$originalPath = $env:Path
$originalEncoding = $OutputEncoding
$env:Path = (($env:Path -split ';' | Where-Object { $_ -notlike '*\MRI*' }) -join ';')
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$wsl = Join-Path $env:SystemRoot 'System32\wsl.exe'
$distro = 'Oculus'

function Invoke-OculusScript([string] $User, [string] $Script) {
    if ($User -notin @('root', 'oculus')) { throw 'Unexpected setup user.' }
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $wsl
    $start.Arguments = "--distribution Oculus --user $User --cd / --exec /bin/bash -s"
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    # Windows PowerShell 5.1's .NET Framework lacks this optional property.
    # The fixed Bash setup payloads are ASCII, so its default encoding is safe.
    if ($start.PSObject.Properties.Name -contains 'StandardInputEncoding') {
        $start.StandardInputEncoding = [System.Text.UTF8Encoding]::new($false)
    }
    $process = [System.Diagnostics.Process]::Start($start)
    try {
        # PowerShell pipelines append CRLF, which can turn the final Bash `fi`
        # into an invalid token. Write the script with exact LF endings instead.
        $process.StandardInput.Write($Script.Replace("`r`n", "`n") + "`n")
        $process.StandardInput.Close()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "Oculus Linux setup failed (exit $($process.ExitCode))." }
    } finally {
        $process.Dispose()
    }
}

try {
    if (-not (Test-Path -LiteralPath $wsl -PathType Leaf)) {
        throw 'WSL is not installed. Install WSL2 with Microsoft''s instructions, restart Windows if requested, and rerun this script.'
    }
    $helpText = ((& $wsl --help) -join "`n") -replace "`0", ''
    if ($helpText -notmatch '--name' -or $helpText -notmatch '--set-default-user') {
        throw 'This script requires a current WSL release with named distributions and --manage --set-default-user. Update WSL and rerun.'
    }
    $names = (((& $wsl --list --quiet) -join "`n") -replace "`0", '') -split "`n" | ForEach-Object { $_.Trim() }
    if ($LASTEXITCODE -ne 0) { throw 'Unable to list WSL distributions.' }
    $created = $names -notcontains $distro
    if ($created) {
        & $wsl --install Ubuntu-24.04 --name $distro --version 2 --no-launch --web-download
        if ($LASTEXITCODE -ne 0) { throw "WSL installation failed (exit $LASTEXITCODE)." }
        Invoke-OculusScript root 'set -eu; install -d -m 0755 /etc/oculus; touch /etc/oculus/managed-wsl'
    } else {
        Invoke-OculusScript root @'
set -eu
if [ ! -f /etc/oculus/managed-wsl ]; then
    printf '%s\n' 'An unrelated WSL distribution is already named Oculus. No changes made.' >&2
    exit 1
fi
'@
    }

    Invoke-OculusScript root @'
set -eu
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export DEBIAN_FRONTEND=noninteractive
if ! id oculus >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash oculus
fi
if [ "$(id -u oculus)" = 0 ]; then
    printf '%s\n' 'The Oculus account must not be root.' >&2
    exit 1
fi
apt-get update -qq
apt-get install -y --no-install-recommends bubblewrap socat libseccomp2 python3 curl ca-certificates unzip ripgrep
python3 - <<'PY'
import configparser
from pathlib import Path
path = Path('/etc/wsl.conf')
config = configparser.ConfigParser()
config.optionxform = str
config.read(path)
if not config.has_section('boot'):
    config.add_section('boot')
# This distribution runs CLI subprocesses only. System services are unnecessary
# and WSL may otherwise wait ten seconds for a systemd user-session cold start.
config.set('boot', 'systemd', 'false')
if not config.has_section('interop'):
    config.add_section('interop')
config.set('interop', 'enabled', 'false')
config.set('interop', 'appendWindowsPath', 'false')
if not config.has_section('user'):
    config.add_section('user')
config.set('user', 'default', 'oculus')
with path.open('w') as stream:
    config.write(stream)
PY
'@
    & $wsl --manage $distro --set-default-user oculus
    if ($LASTEXITCODE -ne 0) { throw 'Unable to set the Oculus Linux user.' }
    # Only this dedicated distribution is restarted; other WSL workloads remain running.
    & $wsl --terminate $distro
    if ($LASTEXITCODE -ne 0) { throw 'Unable to restart the Oculus distribution.' }

    Invoke-OculusScript oculus @'
set -eu
export PATH=/home/oculus/.local/bin:/home/oculus/.bun/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
cd /home/oculus
workdir=$(mktemp -d /tmp/oculus-setup.XXXXXX)
trap 'rm -f "$workdir/claude-install.sh" "$workdir/bun-install.sh"; rmdir "$workdir"' EXIT
if [ ! -x /home/oculus/.local/bin/claude ]; then
    curl -fsSL https://claude.ai/install.sh -o "$workdir/claude-install.sh"
    bash "$workdir/claude-install.sh" stable
fi
if [ ! -x /home/oculus/.bun/bin/bun ]; then
    curl -fsSL https://bun.sh/install -o "$workdir/bun-install.sh"
    bash "$workdir/bun-install.sh"
fi
bun add --global @anthropic-ai/sandbox-runtime@0.0.76
bwrap --ro-bind / / --unshare-pid --die-with-parent /bin/true
python3 -c 'import ctypes; ctypes.CDLL("libseccomp.so.2"); print("libseccomp available")'
claude --version
bwrap --version
if claude auth status --json; then
    printf '%s\n' 'Claude authentication is ready.'
else
    printf '%s\n' 'Claude needs a one-time sign-in. Run the login command printed below.'
fi
'@
    Write-Output 'Setup complete. To sign in interactively, run:'
    Write-Output 'wsl.exe --distribution Oculus --user oculus --cd /home/oculus --exec /home/oculus/.local/bin/claude auth login'
    Write-Output 'The default WSL distribution and other Linux distributions were not changed.'
} finally {
    $env:Path = $originalPath
    $OutputEncoding = $originalEncoding
}
