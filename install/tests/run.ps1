# Tests install.ps1 against a local stand-in for steamcards' GitHub releases,
# and uninstall.ps1 against a stand-in for cargo.
#
#   pwsh -NoProfile -File install/tests/run.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\tests\run.ps1
#
# Each scenario runs install.ps1 or uninstall.ps1 the way `irm ... | iex`
# does, with Invoke-Expression, in a new process of the PowerShell running
# this file, which can't prompt (-NonInteractive). It gets a LOCALAPPDATA,
# APPDATA, USERPROFILE, TEMP and PATH of its own in a temporary folder, and
# the stand-in's address in STEAMCARDS_RELEASES_URL, so it never sees or
# touches a steamcards that's really installed, or your sign-in and
# choices. It prints a line per scenario, and exits with 1 if any failed. It
# needs Python 3.
#
# Installing adds the install folder to the user PATH, and uninstalling takes
# it off. That's in the registry, so a scenario can't have a user PATH of its
# own. Each one puts it back as it was afterwards. Only on CI, on a machine
# that's thrown away after, do the tests set it to a value of their own first.
#
# The scripts are for Windows. On macOS and Linux, pwsh runs the scenarios
# that don't need Windows, and skips the ones that change the user PATH or
# need a running steamcards.exe.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-StrictMode -Version 2.0

$onWindows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
# GitHub Actions sets CI=true.
$onCI = $env:CI -eq 'true'
# Every scenario runs in a new process of this same PowerShell.
$shell = [Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
$installer = Join-Path (Split-Path -Parent $PSScriptRoot) 'install.ps1'
$uninstaller = Join-Path (Split-Path -Parent $PSScriptRoot) 'uninstall.ps1'
$asset = 'steamcards-x86_64-pc-windows-msvc.zip'
$tally = @{ Run = 0; Passed = 0; Failed = 0; Skipped = 0 }

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$work = Join-Path $tempRoot ('steamcards-install-tests-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
$releases = Join-Path $work 'releases'
$builds = Join-Path $work 'builds'

# What each scenario's PowerShell runs: install.ps1 or uninstall.ps1, as
# `irm ... | iex` runs it, with what it throws written plainly to the error
# output. (Run like this, PowerShell would write errors and progress as XML,
# for another PowerShell to read, but for -OutputFormat Text, and the progress
# isn't wanted anyway.)
$childCommand = @'
$ProgressPreference = 'SilentlyContinue'
try {
    Invoke-Expression ([IO.File]::ReadAllText($env:STEAMCARDS_TEST_SCRIPT))
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
exit 0
'@
$encodedCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($childCommand))

# A stand-in for steamcards.exe. On Windows it's a real program, compiled
# with the C# compiler that comes with the .NET Framework, because install.ps1
# runs it. Each version is compiled once, with its version built in, since
# install.ps1 copies only the .exe.
$standInSource = @'
// A stand-in for steamcards.exe, for install.ps1's tests.
using System;
using System.Threading;

static class StandIn
{
    const string Version = "@VERSION@";
    const string Build = "@BUILD@";

    static int Main(string[] args)
    {
        if (args.Length > 0 && args[0] == "--version")
        {
            if (Version.Length == 0)
            {
                // A build from before --version, which doesn't know it.
                Console.Error.WriteLine("error: unexpected argument '--version' found");
                return 2;
            }
            Console.WriteLine("steamcards " + Version);
            return 0;
        }
        if (args.Length > 0 && args[0] == "--wait")
        {
            // Stays running, as steamcards does, until the tests stop it.
            Thread.Sleep(TimeSpan.FromMinutes(5));
            return 0;
        }
        Console.WriteLine("A stand-in for steamcards " + Version + ", " + Build + ".");
        return 0;
    }
}
'@

# On macOS and Linux, a script is enough: install.ps1 only runs it.
$standInScript = @'
#!/bin/sh
# A stand-in for steamcards.exe, for install.ps1's tests: @BUILD@.
if [ "$1" = --version ]; then
    if [ -z "@VERSION@" ]; then
        echo "error: unexpected argument '--version' found" >&2
        exit 2
    fi
    echo "steamcards @VERSION@"
fi
'@

$csc = $null
if ($onWindows) {
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) {
        $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319\csc.exe'
    }
}

# Whether FOLDER has the program NAME in it, like steamcards or cargo. PATH
# can have entries Test-Path can't read, like ones in quotes, and those don't
# count.
function Test-ProgramFolder {
    param([string] $Folder, [string] $Name)
    try {
        (Test-Path -LiteralPath (Join-Path $Folder "$Name.exe")) -or (Test-Path -LiteralPath (Join-Path $Folder $Name))
    } catch {
        $false
    }
}

# The scenarios' PATH: this one, less any folder with a steamcards in it,
# plus whatever a scenario adds.
$separator = [IO.Path]::PathSeparator
$basePath = @($env:PATH.Split($separator) | Where-Object { $_ -and -not (Test-ProgramFolder $_ 'steamcards') })

# Python 3, for the stand-in for GitHub. Windows has python or py, and may
# have a python3 that only offers to install Python from the Store.
function Get-PythonCommand {
    $candidates = @(
        @{ Name = 'python3'; Arguments = @() },
        @{ Name = 'python'; Arguments = @() },
        @{ Name = 'py'; Arguments = @('-3') }
    )
    foreach ($candidate in $candidates) {
        $found = Get-Command $candidate.Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $found) {
            continue
        }
        $arguments = $candidate.Arguments
        try {
            $major = & $found.Path @arguments -c 'import sys; print(sys.version_info[0])' 2>$null
        } catch {
            $major = $null
        }
        if ("$major".Trim() -eq '3') {
            return @{ Path = $found.Path; Arguments = $arguments }
        }
    }
    throw 'The tests need Python 3, as python3, python or py.'
}

# A command line for Start-Process, which takes one as a string.
function ConvertTo-CommandLine {
    param([string[]] $Arguments)
    (@($Arguments) | ForEach-Object { '"' + $_ + '"' }) -join ' '
}

# Write-FakeBinary puts a stand-in for steamcards VERSION at PATH. An empty
# VERSION is a build from before --version. BUILD tells two builds of the same
# version apart.
function Write-FakeBinary {
    param([string] $Version, [string] $Path, [string] $Build = 'release')
    $name = if ($Version) { $Version } else { 'old' }
    $cached = Join-Path (Join-Path $builds "$name-$Build") 'steamcards.exe'
    if (-not (Test-Path -LiteralPath $cached)) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $cached) | Out-Null
        if ($onWindows) {
            $source = [IO.Path]::ChangeExtension($cached, '.cs')
            [IO.File]::WriteAllText($source, $standInSource.Replace('@VERSION@', $Version).Replace('@BUILD@', $Build))
            $compiled = & $csc /nologo /target:exe "/out:$cached" $source
            if ($LASTEXITCODE -ne 0) {
                throw "Couldn't compile a stand-in for steamcards ${name}: $compiled"
            }
        } else {
            $text = $standInScript.Replace("`r`n", "`n").Replace('@VERSION@', $Version).Replace('@BUILD@', $Build)
            [IO.File]::WriteAllText($cached, $text)
            & chmod 755 $cached
        }
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    Copy-Item -LiteralPath $cached -Destination $Path -Force
    if (-not $onWindows) {
        & chmod 755 $Path
    }
}

# Where the release build of VERSION is kept, to compare with what's
# installed.
function Get-BuildPath {
    param([string] $Version)
    Join-Path (Join-Path $builds "$Version-release") 'steamcards.exe'
}

# Write-FakeCargo puts a stand-in for cargo in FOLDER: it notes each command
# it's given in the scenario's Ran file, and does nothing else. On Windows
# it's a batch file, which Get-Command finds as cargo, as it would cargo.exe.
function Write-FakeCargo {
    param($Scenario, [string] $Folder)
    New-Item -ItemType Directory -Force -Path $Folder | Out-Null
    if ($onWindows) {
        [IO.File]::WriteAllText((Join-Path $Folder 'cargo.cmd'), "@echo off`r`n>>`"$($Scenario.Ran)`" echo cargo %*`r`n")
    } else {
        $path = Join-Path $Folder 'cargo'
        [IO.File]::WriteAllText($path, "#!/bin/sh`n# A stand-in for cargo, for uninstall.ps1's tests.`necho `"cargo `$*`" >>'$($Scenario.Ran)'`n")
        & chmod 755 $path
    }
}

# Write-FakeConfig puts sign-in and choices in FOLDER, as steamcards does,
# with a debug log beside them.
function Write-FakeConfig {
    param([string] $Folder)
    New-Item -ItemType Directory -Force -Path $Folder | Out-Null
    [IO.File]::WriteAllText((Join-Path $Folder 'config.json'), "{`"accounts`": {}, `"preferences`": {}}`n")
    [IO.File]::WriteAllText((Join-Path $Folder 'debug.log'), "A stand-in for steamcards' debug log.`n")
}

# Publish-FakeRelease makes a release like the real ones: the Windows zip
# (steamcards.exe, LICENSE and README.md), and SHA256SUMS, which lists the
# other platforms' archives too.
function Publish-FakeRelease {
    param([string] $Version)
    $staging = Join-Path $work "staging-$Version"
    $folder = Join-Path $releases "v$Version"
    New-Item -ItemType Directory -Force -Path $staging, $folder | Out-Null
    Write-FakeBinary -Version $Version -Path (Join-Path $staging 'steamcards.exe')
    [IO.File]::WriteAllText((Join-Path $staging 'LICENSE'), "A stand-in for steamcards' licence.`n")
    [IO.File]::WriteAllText((Join-Path $staging 'README.md'), "A stand-in for steamcards $Version.`n")
    $zip = Join-Path $folder $asset
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLowerInvariant()
    $zeros = '0' * 64
    [IO.File]::WriteAllText((Join-Path $folder 'SHA256SUMS'), "$zeros  steamcards-x86_64-unknown-linux-musl.tar.gz`n$hash  $asset`n")
}

# Write-LatestRelease chooses the release /latest redirects to. An empty
# VERSION is a repository with no releases, which GitHub redirects to its
# releases page, and -Missing makes /latest a 404, like a private repository.
function Write-LatestRelease {
    param([string] $Version, [switch] $Missing)
    $file = Join-Path $releases 'latest'
    if ($Missing) {
        Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
    } else {
        [IO.File]::WriteAllText($file, "$Version`n")
    }
}

# The user PATH as the registry has it, unexpanded, with its kind, or $null if
# there's none. Windows only.
function Get-UserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        if ($key.GetValueNames() -notcontains 'Path') {
            return $null
        }
        [pscustomobject]@{
            Value = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            Kind = $key.GetValueKind('Path')
        }
    } finally {
        $key.Close()
    }
}

# Write-UserPath puts SAVED, from Get-UserPath, in the registry.
function Write-UserPath {
    param($Saved)
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
        if ($null -eq $Saved) {
            $key.DeleteValue('Path', $false)
        } else {
            $key.SetValue('Path', $Saved.Value, $Saved.Kind)
        }
    } finally {
        $key.Close()
    }
}

function Format-UserPath {
    param($Saved)
    if ($null -eq $Saved) {
        return '(none)'
    }
    "$($Saved.Kind) $($Saved.Value)"
}

# Add-UserPathEntry puts SCENARIO's install folder on the user PATH, as
# install.ps1 does, and returns the user PATH as it should be once it's taken
# off again. On CI, the user PATH is the tests' own, with the folder between
# entries that have to stay as they are: one unexpanded, and an empty one.
# Elsewhere, it's yours, with the folder on the end. Windows only.
function Add-UserPathEntry {
    param($Scenario)
    if ($onCI) {
        $expected = [pscustomobject]@{
            Value = '%USERPROFILE%\steamcards-tests;;C:\steamcards-tests'
            Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
        }
        $value = "%USERPROFILE%\steamcards-tests;$($Scenario.InstallDir);;C:\steamcards-tests"
    } else {
        $expected = Get-UserPath
        if ($null -eq $expected) {
            $expected = [pscustomobject]@{ Value = ''; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
        }
        $value = "$($expected.Value);$($Scenario.InstallDir)"
    }
    Write-UserPath ([pscustomobject]@{ Value = $value; Kind = $expected.Kind })
    $expected
}

# Invoke-Installer runs install.ps1 for SCENARIO, and Invoke-Uninstaller
# uninstall.ps1, in a new process with the scenario's environment and
# ENVIRONMENT's variables ($null removes one). Each returns the script's exit
# code and output.
function Invoke-Installer {
    param($Scenario, [hashtable] $Environment = @{})
    Invoke-ScenarioScript -Scenario $Scenario -Script $installer -Environment $Environment
}

function Invoke-Uninstaller {
    param($Scenario, [hashtable] $Environment = @{})
    Invoke-ScenarioScript -Scenario $Scenario -Script $uninstaller -Environment $Environment
}

function Invoke-ScenarioScript {
    param($Scenario, [string] $Script, [hashtable] $Environment)
    $variables = @{
        STEAMCARDS_TEST_SCRIPT = $Script
        STEAMCARDS_RELEASES_URL = $url
        STEAMCARDS_VERSION = $null
        STEAMCARDS_FORCE = $null
        STEAMCARDS_DELETE_DATA = $null
        CARGO_HOME = $null
        LOCALAPPDATA = $Scenario.LocalAppData
        APPDATA = $Scenario.AppData
        USERPROFILE = $Scenario.Home
        HOME = $Scenario.Home
        TEMP = $Scenario.Temp
        TMP = $Scenario.Temp
        TMPDIR = $Scenario.Temp
        PATH = $Scenario.Path -join $separator
        POWERSHELL_TELEMETRY_OPTOUT = '1'
        POWERSHELL_UPDATECHECK = 'Off'
    }
    if (-not $onWindows) {
        # install.ps1 checks it's on 64-bit Windows.
        $variables['PROCESSOR_ARCHITECTURE'] = 'AMD64'
        $variables['PROCESSOR_ARCHITEW6432'] = $null
    }
    foreach ($name in $Environment.Keys) {
        $variables[$name] = $Environment[$name]
    }

    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $shell
    $start.Arguments = "-NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encodedCommand"
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($name in $variables.Keys) {
        if ($null -eq $variables[$name]) {
            $start.EnvironmentVariables.Remove($name)
        } else {
            $start.EnvironmentVariables[$name] = $variables[$name]
        }
    }
    $process = [Diagnostics.Process]::Start($start)
    $output = $process.StandardOutput.ReadToEndAsync()
    $errorText = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(120000)) {
        $process.Kill()
        throw "$(Split-Path -Leaf $Script) was still running after two minutes."
    }
    $Scenario.Result = [pscustomobject]@{
        ExitCode = $process.ExitCode
        Output = $output.Result
        Errors = $errorText.Result
    }
    $Scenario.Result
}

# What a steamcards says it is, like "steamcards 0.2.0".
function Get-ReportedVersion {
    param([string] $Path)
    try {
        if ($onWindows) {
            $said = & $Path --version 2>$null
        } else {
            # Unzipped, a stand-in isn't executable any more, but sh runs it.
            $said = & sh $Path --version 2>$null
        }
    } catch {
        $said = $null
    }
    "$said".Trim()
}

# Whether a line of TEXT is like PATTERN, a wildcard pattern that minds case.
# Lock-Folder and Unlock-Folder: stop the tests' own user adding files to
# FOLDER, and let it again. On Windows, with a rule that denies it (which
# binds an administrator too); elsewhere, with the folder's mode.
function Lock-Folder {
    param([string] $Folder)
    if ($onWindows) {
        # Only adding files (WD) and folders (AD): the whole of write (W)
        # takes the right to open the folder too, which hides what's in it.
        $user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
        icacls $Folder /deny "${user}:(WD,AD)" | Out-Null
    } else {
        chmod 555 $Folder
    }
    if ($LASTEXITCODE -ne 0) { throw "couldn't lock $Folder" }
}

function Unlock-Folder {
    param([string] $Folder)
    if ($onWindows) {
        $user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
        icacls $Folder /remove:d $user | Out-Null
    } else {
        chmod 755 $Folder
    }
    if ($LASTEXITCODE -ne 0) { throw "couldn't unlock $Folder" }
}

function Test-Line {
    param([string] $Text, [string] $Pattern)
    foreach ($line in ($Text -split '\r?\n')) {
        if ($line -clike $Pattern) {
            return $true
        }
    }
    $false
}

function Assert-ExitCode {
    param($Result, [int] $Expected)
    if ($Result.ExitCode -ne $Expected) {
        throw "exit code $($Result.ExitCode), expected $Expected"
    }
}

function Assert-OutputLine {
    param($Result, [string] $Pattern)
    if (-not (Test-Line -Text $Result.Output -Pattern $Pattern)) {
        throw "no line of output like: $Pattern"
    }
}

function Assert-NoOutputLine {
    param($Result, [string] $Pattern)
    if (Test-Line -Text $Result.Output -Pattern $Pattern) {
        throw "a line of output like: $Pattern"
    }
}

function Assert-ErrorLine {
    param($Result, [string] $Pattern)
    if (-not (Test-Line -Text $Result.Errors -Pattern $Pattern)) {
        throw "no line of error output like: $Pattern"
    }
}

function Assert-NoErrorOutput {
    param($Result)
    if ((Get-ErrorOutput $Result).Trim()) {
        throw 'error output, where there should be none'
    }
}

# Get-ErrorOutput: what RESULT wrote as errors. Windows PowerShell 5.1, with
# its output redirected (as here, though never when someone pastes the
# one-liner in), also writes its host messages to the error stream, as CLIXML
# "information" records: the Write-Host lines, which are in its output too.
# Those aren't errors. An error record is marked S="Error".
function Get-ErrorOutput {
    param($Result)
    $text = "$($Result.Errors)"
    if ($text -match '^\s*#< CLIXML' -and $text -notmatch 'S="Error"') {
        return ''
    }
    $text
}

# Assert-SameFile: PATH is a copy of EXPECTED.
function Assert-SameFile {
    param([string] $Path, [string] $Expected)
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Path isn't there"
    }
    if ((Get-FileHash -LiteralPath $Path).Hash -ne (Get-FileHash -LiteralPath $Expected).Hash) {
        throw "$Path isn't the same as $Expected"
    }
}

function Assert-Version {
    param([string] $Path, [string] $Version)
    $said = Get-ReportedVersion -Path $Path
    if ($said -ne "steamcards $Version") {
        throw "$Path says `"$said`", not `"steamcards $Version`""
    }
}

function Assert-Missing {
    param([string] $Path)
    if (Test-Path -LiteralPath $Path) {
        throw "$Path is there, and shouldn't be"
    }
}

function Assert-Present {
    param([string] $Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Path isn't there, and should be"
    }
}

# Assert-Ran: the stand-in for cargo was given these COMMANDS, in this order,
# and no others.
function Assert-Ran {
    param($Scenario, [string[]] $Commands)
    $ran = @()
    if (Test-Path -LiteralPath $Scenario.Ran) {
        $ran = @(Get-Content -LiteralPath $Scenario.Ran)
    }
    if (($ran -join "`n") -cne ($Commands -join "`n")) {
        throw "cargo was given: $($ran -join '; '), where it should have been: $($Commands -join '; ')"
    }
}

# Assert-Tidy: install.ps1 left nothing behind, in FOLDER or in its TEMP.
function Assert-Tidy {
    param($Scenario, [string] $Folder)
    if (Test-Path -LiteralPath (Join-Path $Folder 'steamcards.exe.new')) {
        throw "left behind: steamcards.exe.new in $Folder"
    }
    $left = @(Get-ChildItem -LiteralPath $Scenario.Temp -Filter 'steamcards-*' -Force)
    if ($left.Count -gt 0) {
        throw "left behind in TEMP: $(($left | ForEach-Object { $_.Name }) -join ', ')"
    }
}

function Assert-UserPath {
    param($Expected)
    $actual = Format-UserPath (Get-UserPath)
    if ($actual -ne (Format-UserPath $Expected)) {
        throw "the user PATH is $actual, expected $(Format-UserPath $Expected)"
    }
}

# Wait-ProcessPath waits until PROCESS, just started from PATH, shows that
# path, which is what install.ps1 looks for.
function Wait-ProcessPath {
    param([Diagnostics.Process] $Process, [string] $Path)
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        try {
            $seen = (Get-Process -Id $Process.Id).Path
        } catch {
            $seen = $null
        }
        if ($seen -eq $Path) {
            return
        }
        Start-Sleep -Milliseconds 100
    }
    throw "The stand-in started from $Path didn't show that path."
}

# Invoke-Scenario runs BODY with a scenario of its own:
#   Root, Home, Temp, LocalAppData, AppData   its folders
#   InstallDir, Exe                           where install.ps1 installs to
#   Data                                      where steamcards keeps your
#                                             sign-in and choices
#   Ran                                       what the stand-in for cargo notes
#   Path                                      its PATH, as a list
#   Result                                    the script's last result
# and the latest release at 0.2.0.
function Invoke-Scenario {
    param([string] $Name, [scriptblock] $Body, [switch] $WindowsOnly)
    $tally.Run++
    if ($WindowsOnly -and -not $onWindows) {
        $tally.Skipped++
        Write-Host "skip  $Name (Windows only)"
        return
    }
    $root = Join-Path $work "scenario-$($tally.Run)"
    $localAppData = Join-Path $root 'LocalAppData'
    $appData = Join-Path $root 'AppData'
    $installDir = Join-Path $localAppData 'Programs\steamcards'
    $scenario = [pscustomobject]@{
        Root = $root
        Home = Join-Path $root 'home'
        Temp = Join-Path $root 'temp'
        LocalAppData = $localAppData
        AppData = $appData
        InstallDir = $installDir
        Exe = Join-Path $installDir 'steamcards.exe'
        Data = Join-Path $appData 'steamcards'
        Ran = Join-Path $root 'ran'
        Path = @($basePath)
        Result = $null
    }
    New-Item -ItemType Directory -Path $scenario.Home, $scenario.Temp, $scenario.LocalAppData, $scenario.AppData | Out-Null
    Write-LatestRelease -Version '0.2.0'
    $userPath = $null
    if ($onWindows) {
        $userPath = Get-UserPath
    }
    try {
        & $Body $scenario
        $tally.Passed++
        Write-Host "ok    $Name"
    } catch {
        $tally.Failed++
        Write-Host "FAIL  $Name"
        Write-Host "    $($_.Exception.Message)"
        foreach ($line in ($_.ScriptStackTrace -split '\r?\n')) {
            Write-Host "    $line"
        }
        if ($null -ne $scenario.Result) {
            Write-Host "    exit code: $($scenario.Result.ExitCode)"
            foreach ($line in ("$($scenario.Result.Output)" -split '\r?\n')) {
                if ($line) { Write-Host "    out: $line" }
            }
            foreach ($line in ("$($scenario.Result.Errors)" -split '\r?\n')) {
                if ($line) { Write-Host "    err: $line" }
            }
        }
    } finally {
        if ($onWindows -and (Format-UserPath (Get-UserPath)) -ne (Format-UserPath $userPath)) {
            Write-UserPath $userPath
            # Tells Windows the environment changed, as install.ps1 does, so
            # nothing keeps the scenario's PATH.
            [Environment]::SetEnvironmentVariable('STEAMCARDS_TESTS', '1', 'User')
            [Environment]::SetEnvironmentVariable('STEAMCARDS_TESTS', $null, 'User')
        }
    }
}

$server = $null
$exitCode = 1
New-Item -ItemType Directory -Path $work, $releases, $builds | Out-Null
try {
    # The stand-in for GitHub (server.py says what it serves), on a free port.
    $python = Get-PythonCommand
    $portFile = Join-Path $work 'port'
    $serverLog = Join-Path $work 'server.log'
    $serverArguments = @($python.Arguments) + @((Join-Path $PSScriptRoot 'server.py'), $releases, $portFile)
    $server = Start-Process -FilePath $python.Path -ArgumentList (ConvertTo-CommandLine $serverArguments) -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $work 'server.out') -RedirectStandardError $serverLog
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while (-not (Test-Path -LiteralPath $portFile)) {
        if ($server.HasExited -or [DateTime]::UtcNow -gt $deadline) {
            throw "The stand-in for GitHub didn't start. $(Get-Content -Raw -LiteralPath $serverLog -ErrorAction SilentlyContinue)"
        }
        Start-Sleep -Milliseconds 100
    }
    $url = 'http://127.0.0.1:' + (Get-Content -Raw -LiteralPath $portFile).Trim() + '/releases'
    Write-Host "Testing install.ps1 against a stand-in for GitHub at $url, and uninstall.ps1, under PowerShell $($PSVersionTable.PSVersion) ($($PSVersionTable.PSEdition))."

    foreach ($version in '0.1.0', '0.2.0', '0.3.0', '0.4.0', '0.10.0') {
        Publish-FakeRelease -Version $version
    }
    # 0.4.0's zip is swapped for 0.3.0's after its SHA256SUMS was written: a
    # real zip, which doesn't match its checksum.
    Copy-Item -LiteralPath (Join-Path (Join-Path $releases 'v0.3.0') $asset) -Destination (Join-Path (Join-Path $releases 'v0.4.0') $asset) -Force

    Invoke-Scenario -Name 'a fresh install, which adds the folder to the user PATH' -WindowsOnly -Body {
        param($s)
        if ($onCI) {
            # Only on CI: a user PATH of the tests' own, with an entry that has
            # to stay as it is, unexpanded, and an empty one.
            Write-UserPath ([pscustomobject]@{
                    Value = '%USERPROFILE%\steamcards-tests;;C:\steamcards-tests'
                    Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
                })
        }
        $before = Get-UserPath
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Downloading steamcards 0.2.0...'
        Assert-OutputLine $r "Installed steamcards 0.2.0 to $($s.Exe)."
        Assert-OutputLine $r "Added $($s.InstallDir) to your PATH. Restart your terminal, then run: steamcards"
        Assert-NoErrorOutput $r
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
        Assert-Tidy $s $s.InstallDir
        # The folder goes on the end, and the rest stays as it was, less any
        # empty entries.
        $entries = @()
        if ($null -ne $before) {
            $entries = @($before.Value.Split(';') | Where-Object { $_ })
        }
        Assert-UserPath ([pscustomobject]@{
                Value = (@($entries) + $s.InstallDir) -join ';'
                Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
            })
    }

    Invoke-Scenario -Name 'already up to date' -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'steamcards 0.2.0 is already up to date.'
        Assert-NoOutputLine $r 'Downloading*'
        Assert-NoErrorOutput $r
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
    }

    Invoke-Scenario -Name 'an update' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.1.0' -Path $s.Exe
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Updated steamcards 0.1.0 * 0.2.0.'
        Assert-NoOutputLine $r 'Run it with: steamcards'
        Assert-NoErrorOutput $r
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
        Assert-Version $s.Exe '0.2.0'
        Assert-Tidy $s $s.InstallDir
    }

    Invoke-Scenario -Name 'a pinned downgrade, with STEAMCARDS_VERSION=v0.1.0' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        $r = Invoke-Installer $s @{ STEAMCARDS_VERSION = 'v0.1.0' }
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Downloading steamcards 0.1.0...'
        Assert-OutputLine $r 'Downgraded steamcards 0.2.0 * 0.1.0.'
        Assert-SameFile $s.Exe (Get-BuildPath '0.1.0')
    }

    Invoke-Scenario -Name 'a forced reinstall, with STEAMCARDS_FORCE=1' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe -Build 'local'
        $r = Invoke-Installer $s @{ STEAMCARDS_FORCE = '1' }
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Reinstalled steamcards 0.2.0.'
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
    }

    Invoke-Scenario -Name 'an old build without --version, replaced' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '' -Path $s.Exe
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Updated steamcards to 0.2.0.'
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
    }

    Invoke-Scenario -Name 'a copy elsewhere on PATH, updated where it is' -Body {
        param($s)
        $tools = Join-Path $s.Root 'tools'
        $exe = Join-Path $tools 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        $s.Path = @($tools) + $s.Path
        $before = ''
        if ($onWindows) {
            $before = Format-UserPath (Get-UserPath)
        }
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Updated steamcards 0.1.0 * 0.2.0.'
        Assert-NoOutputLine $r 'Added *'
        Assert-NoErrorOutput $r
        Assert-SameFile $exe (Get-BuildPath '0.2.0')
        Assert-Missing $s.Exe
        Assert-Tidy $s $tools
        if ($onWindows -and (Format-UserPath (Get-UserPath)) -ne $before) {
            throw 'the user PATH changed'
        }
    }

    Invoke-Scenario -Name "a copy in a folder you can't write to, refused" -Body {
        param($s)
        $locked = Join-Path $s.Root 'locked'
        $exe = Join-Path $locked 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        $s.Path = @($locked) + $s.Path
        Lock-Folder -Folder $locked
        try {
            $r = Invoke-Installer $s
        } finally {
            Unlock-Folder -Folder $locked
        }
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "steamcards is in $locked, which only an administrator can change. Update it the way it was installed, or delete it and run this command again to install it to $($s.InstallDir)."
        Assert-SameFile $exe (Get-BuildPath '0.1.0')
        Assert-Missing $s.Exe
    }

    Invoke-Scenario -Name 'a copy that cargo installed, refused' -Body {
        param($s)
        $cargoBin = Join-Path (Join-Path $s.Home '.cargo') 'bin'
        $exe = Join-Path $cargoBin 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        $s.Path = @($cargoBin) + $s.Path
        $r = Invoke-Installer $s
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "$exe was built from source with cargo. Update it the same way: https://github.com/joshgallantt/steamcards/blob/main/CONTRIBUTING.md#building-from-source"
        Assert-SameFile $exe (Get-BuildPath '0.1.0')
        Assert-Missing $s.Exe
    }

    Invoke-Scenario -Name 'a tampered zip, refused, with nothing replaced' -Body {
        param($s)
        Write-FakeBinary -Version '0.1.0' -Path $s.Exe
        Write-LatestRelease -Version '0.4.0'
        $r = Invoke-Installer $s
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "$asset doesn't match its checksum in SHA256SUMS, so nothing was installed."
        Assert-SameFile $s.Exe (Get-BuildPath '0.1.0')
        Assert-Tidy $s $s.InstallDir
    }

    Invoke-Scenario -Name 'no release yet: /latest is a 404' -Body {
        param($s)
        Write-LatestRelease -Missing
        $r = Invoke-Installer $s
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "Couldn't find a release at $url (*)"
        Assert-Missing $s.Exe
    }

    Invoke-Scenario -Name 'no release yet: /latest redirects to the releases page' -Body {
        param($s)
        Write-LatestRelease -Version ''
        $r = Invoke-Installer $s
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "Couldn't find a release at $url"
        Assert-Missing $s.Exe
    }

    Invoke-Scenario -Name 'a copy newer than the latest release, left alone' -Body {
        param($s)
        Write-FakeBinary -Version '0.3.0' -Path $s.Exe
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'steamcards 0.3.0 is newer than the latest release, 0.2.0. Leaving it as it is.'
        Assert-NoErrorOutput $r
        Assert-SameFile $s.Exe (Get-BuildPath '0.3.0')
    }

    Invoke-Scenario -Name 'steamcards running, refused' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.1.0' -Path $s.Exe
        $running = Start-Process -FilePath $s.Exe -ArgumentList '--wait' -PassThru -WindowStyle Hidden
        try {
            Wait-ProcessPath -Process $running -Path $s.Exe
            $r = Invoke-Installer $s
            Assert-ExitCode $r 1
            Assert-ErrorLine $r 'steamcards is running. Close it, then run this command again.'
            Assert-SameFile $s.Exe (Get-BuildPath '0.1.0')
            Assert-Tidy $s $s.InstallDir
        } finally {
            Stop-Process -Id $running.Id -Force -ErrorAction SilentlyContinue
            $running.WaitForExit()
        }
    }

    Invoke-Scenario -Name '0.9.0 updated to 0.10.0, as numbers compare' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.9.0' -Path $s.Exe
        Write-LatestRelease -Version '0.10.0'
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Updated steamcards 0.9.0 * 0.10.0.'
        Assert-SameFile $s.Exe (Get-BuildPath '0.10.0')
    }

    Invoke-Scenario -Name 'a pre-release, updated to its release' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0-rc.1' -Path $s.Exe
        $r = Invoke-Installer $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Updated steamcards 0.2.0-rc.1 * 0.2.0.'
        Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
    }

    Invoke-Scenario -Name "a pinned version that doesn't exist" -Body {
        param($s)
        Write-FakeBinary -Version '0.1.0' -Path $s.Exe
        $r = Invoke-Installer $s @{ STEAMCARDS_VERSION = '9.9.9' }
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "Couldn't download steamcards 9.9.9 from $url/download/v9.9.9 (*)"
        Assert-SameFile $s.Exe (Get-BuildPath '0.1.0')
        Assert-Tidy $s $s.InstallDir
    }

    Invoke-Scenario -Name 'STEAMCARDS_RELEASES_URL ending in /' -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        $r = Invoke-Installer $s @{ STEAMCARDS_RELEASES_URL = "$url/" }
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'steamcards 0.2.0 is already up to date.'
    }

    Invoke-Scenario -Name 'uninstall: the install folder, deleted' -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        # Left behind by an update that was stopped, say.
        Set-Content -LiteralPath (Join-Path $s.InstallDir 'steamcards.exe.new') -Value ''
        $before = ''
        if ($onWindows) {
            $before = Format-UserPath (Get-UserPath)
        }
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r "Deleting $($s.InstallDir)."
        Assert-OutputLine $r 'Uninstalled steamcards.'
        Assert-NoErrorOutput $r
        Assert-Missing $s.InstallDir
        Assert-Present (Split-Path -Parent $s.InstallDir)
        if ($onWindows -and (Format-UserPath (Get-UserPath)) -ne $before) {
            throw 'the user PATH changed'
        }
    }

    Invoke-Scenario -Name "uninstall: your sign-in and choices kept, where it can't ask" -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        Write-FakeConfig $s.Data
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r "Uninstalled steamcards. Your sign-in and choices are still in $($s.Data)."
        Assert-NoErrorOutput $r
        Assert-Missing $s.InstallDir
        Assert-Present (Join-Path $s.Data 'config.json')
    }

    Invoke-Scenario -Name 'uninstall: STEAMCARDS_DELETE_DATA=1 deletes them, with nothing else in that folder' -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        Write-FakeConfig $s.Data
        # Another app's, beside them.
        $other = Join-Path $s.AppData 'another-app'
        Write-FakeConfig $other
        $r = Invoke-Uninstaller $s @{ STEAMCARDS_DELETE_DATA = '1' }
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Uninstalled steamcards and deleted your sign-in and choices.'
        Assert-NoErrorOutput $r
        Assert-Missing $s.Data
        Assert-Present (Join-Path $other 'config.json')
    }

    Invoke-Scenario -Name 'uninstall: nothing installed' -Body {
        param($s)
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r "steamcards isn't installed, so there's nothing to remove."
        Assert-NoErrorOutput $r
    }

    Invoke-Scenario -Name 'uninstall: nothing installed, but sign-in and choices, deleted with STEAMCARDS_DELETE_DATA=1' -Body {
        param($s)
        Write-FakeConfig $s.Data
        $r = Invoke-Uninstaller $s @{ STEAMCARDS_DELETE_DATA = '1' }
        Assert-ExitCode $r 0
        Assert-OutputLine $r "steamcards isn't installed."
        Assert-OutputLine $r 'Deleted your sign-in and choices.'
        Assert-NoErrorOutput $r
        Assert-Missing $s.Data
    }

    Invoke-Scenario -Name 'uninstall: a copy elsewhere on PATH, just steamcards.exe deleted' -Body {
        param($s)
        $tools = Join-Path $s.Root 'tools'
        $exe = Join-Path $tools 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        [IO.File]::WriteAllText((Join-Path $tools 'another-program.txt'), "Another program.`n")
        $s.Path = @($tools) + $s.Path
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r "Deleting $exe."
        Assert-OutputLine $r 'Uninstalled steamcards.'
        Assert-NoErrorOutput $r
        Assert-Missing $exe
        Assert-Present (Join-Path $tools 'another-program.txt')
    }

    Invoke-Scenario -Name 'uninstall: a copy that cargo installed, uninstalled with cargo' -Body {
        param($s)
        $cargoBin = Join-Path (Join-Path $s.Home '.cargo') 'bin'
        $exe = Join-Path $cargoBin 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        Write-FakeCargo $s $cargoBin
        $s.Path = @($cargoBin) + $s.Path
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'steamcards was built from source with cargo. Uninstalling it with: cargo uninstall steamcards'
        Assert-OutputLine $r 'Uninstalled steamcards.'
        Assert-NoErrorOutput $r
        Assert-Ran $s 'cargo uninstall steamcards'
        # cargo's to remove, so that it forgets it too.
        Assert-SameFile $exe (Get-BuildPath '0.1.0')
    }

    Invoke-Scenario -Name 'uninstall: a copy that cargo installed, with no cargo to be found, refused' -Body {
        param($s)
        $cargoBin = Join-Path (Join-Path $s.Home '.cargo') 'bin'
        $exe = Join-Path $cargoBin 'steamcards.exe'
        Write-FakeBinary -Version '0.1.0' -Path $exe
        # No cargo, even where this machine has one.
        $s.Path = @($cargoBin) + @($s.Path | Where-Object { -not (Test-ProgramFolder $_ 'cargo') })
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 1
        Assert-ErrorLine $r "steamcards was built from source with cargo, but cargo isn't on your PATH. Uninstall it with: cargo uninstall steamcards"
        Assert-SameFile $exe (Get-BuildPath '0.1.0')
    }

    Invoke-Scenario -Name 'uninstall: the install folder taken off the user PATH, with the rest kept as it was' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        $expected = Add-UserPathEntry $s
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r "Deleting $($s.InstallDir)."
        Assert-OutputLine $r 'Uninstalled steamcards.'
        Assert-NoErrorOutput $r
        Assert-Missing $s.InstallDir
        Assert-UserPath $expected
    }

    Invoke-Scenario -Name 'uninstall: the install folder left on the user PATH, after steamcards was deleted by hand' -WindowsOnly -Body {
        param($s)
        $expected = Add-UserPathEntry $s
        $r = Invoke-Uninstaller $s
        Assert-ExitCode $r 0
        Assert-OutputLine $r 'Uninstalled steamcards.'
        Assert-NoOutputLine $r "steamcards isn't installed*"
        Assert-NoErrorOutput $r
        Assert-UserPath $expected
    }

    Invoke-Scenario -Name 'uninstall: steamcards running, refused, with nothing changed' -WindowsOnly -Body {
        param($s)
        Write-FakeBinary -Version '0.2.0' -Path $s.Exe
        Write-FakeConfig $s.Data
        Add-UserPathEntry $s | Out-Null
        $before = Format-UserPath (Get-UserPath)
        $running = Start-Process -FilePath $s.Exe -ArgumentList '--wait' -PassThru -WindowStyle Hidden
        try {
            Wait-ProcessPath -Process $running -Path $s.Exe
            $r = Invoke-Uninstaller $s @{ STEAMCARDS_DELETE_DATA = '1' }
            Assert-ExitCode $r 1
            Assert-ErrorLine $r 'steamcards is running. Close it, then run this command again.'
            Assert-SameFile $s.Exe (Get-BuildPath '0.2.0')
            Assert-Present (Join-Path $s.Data 'config.json')
            if ((Format-UserPath (Get-UserPath)) -ne $before) {
                throw 'the user PATH changed'
            }
        } finally {
            Stop-Process -Id $running.Id -Force -ErrorAction SilentlyContinue
            $running.WaitForExit()
        }
    }

    Write-Host ''
    Write-Host "$($tally.Passed) passed, $($tally.Failed) failed, $($tally.Skipped) skipped."
    $exitCode = if ($tally.Failed -gt 0) { 1 } else { 0 }
} finally {
    if ($null -ne $server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
        $server.WaitForExit()
    }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
exit $exitCode
