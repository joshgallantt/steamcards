# Installs or updates steamcards on Windows, from its GitHub releases.
#
#   irm https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/install.ps1 | iex
#
# Run it again to update: it compares your version with the latest release,
# and only downloads when the release is newer. All optional:
#
#   $env:STEAMCARDS_VERSION       the release to install, like 0.2.0 (default: the latest)
#   $env:STEAMCARDS_FORCE=1       reinstall, even if that version is already installed
#   $env:STEAMCARDS_RELEASES_URL  a mirror to download from (default:
#                                  https://github.com/joshgallantt/steamcards/releases),
#                                  laid out the same way: <url>/latest redirects to
#                                  <url>/tag/vX.Y.Z, and the files are in <url>/download/vX.Y.Z/
#
# It installs to %LOCALAPPDATA%\Programs\steamcards (or updates steamcards
# where it already is) and adds that folder to your user PATH. No admin
# rights needed.

# In a script block, so nothing it sets is left behind in your session.
& {
    $ErrorActionPreference = 'Stop'
    # Downloads are far slower with Windows PowerShell's progress bar showing.
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 may not offer TLS 1.2 by itself, and GitHub needs it.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'joshgallantt/steamcards'
    # Where the releases are: GitHub, or a mirror laid out the same way.
    $releases = "https://github.com/$repo/releases"
    if ($env:STEAMCARDS_RELEASES_URL) {
        $releases = $env:STEAMCARDS_RELEASES_URL.TrimEnd('/')
    }
    $asset = 'steamcards-x86_64-pc-windows-msvc.zip'
    $installDir = Join-Path $env:LOCALAPPDATA 'Programs\steamcards'

    # -1, 0 or 1 as version $a is older than, the same as, or newer than
    # version $b, by semver's rules: numbers compare as numbers, and a
    # pre-release (1.0.0-rc.1) comes before its release.
    function Compare-Version([string] $a, [string] $b) {
        $aCore, $aPre = ($a -replace '\+.*$', '').Split('-', 2)
        $bCore, $bPre = ($b -replace '\+.*$', '').Split('-', 2)
        $c = Compare-Identifier $aCore $bCore
        if ($c -ne 0) { return $c }
        if ($aPre -eq $bPre) { return 0 }
        if (-not $aPre) { return 1 }
        if (-not $bPre) { return -1 }
        return Compare-Identifier $aPre $bPre
    }

    # Compares dot-separated identifiers, one at a time.
    function Compare-Identifier([string] $a, [string] $b) {
        $xs = $a.Split('.')
        $ys = $b.Split('.')
        for ($i = 0; $i -lt $xs.Count -and $i -lt $ys.Count; $i++) {
            $xNumber = $xs[$i] -match '^\d+$'
            $yNumber = $ys[$i] -match '^\d+$'
            if ($xNumber -and $yNumber) {
                $c = ([long] $xs[$i]).CompareTo([long] $ys[$i])
            } elseif ($xNumber) {
                $c = -1
            } elseif ($yNumber) {
                $c = 1
            } else {
                $c = [string]::CompareOrdinal($xs[$i], $ys[$i])
            }
            if ($c -ne 0) { return [Math]::Sign($c) }
        }
        return [Math]::Sign($xs.Count - $ys.Count)
    }

    # Which build. Windows 11 on ARM runs the x64 one through emulation.
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'ARM64') {
        Write-Host 'There is no ARM64 build yet, so this installs the x64 one, which Windows 11 runs through emulation.'
    } elseif ($arch -ne 'AMD64') {
        throw "There's no prebuilt steamcards for $arch Windows. See https://github.com/$repo/blob/main/CONTRIBUTING.md#building-from-source"
    }

    # Which version.
    if ($env:STEAMCARDS_VERSION) {
        $version = $env:STEAMCARDS_VERSION.TrimStart('v')
    } else {
        # The latest release's page redirects to its tag: .../releases/tag/v0.2.0.
        # Windows PowerShell 5.1 and PowerShell 7 both follow the redirect, but
        # say where it ended in different places: 5.1's response is an
        # HttpWebResponse, with a ResponseUri, and 7's is an HttpResponseMessage,
        # whose RequestMessage is the last request made. So this reads whichever
        # one is there. (Stopping at the redirect isn't the same in both: 7 makes
        # it an error unless given -SkipHttpErrorCheck, which 5.1 doesn't have.)
        try {
            $response = (Invoke-WebRequest -UseBasicParsing -Method Head -Uri "$releases/latest").BaseResponse
            $landed = if ($response.PSObject.Properties['ResponseUri']) {
                $response.ResponseUri
            } else {
                $response.RequestMessage.RequestUri
            }
        } catch {
            throw "Couldn't find a release at $releases ($($_.Exception.Message))"
        }
        # With no releases yet, GitHub redirects to the releases page instead.
        if ("$landed" -match '/tag/v([^/]+)$') {
            $version = $Matches[1]
        } else {
            throw "Couldn't find a release at $releases"
        }
    }
    if ($version -notmatch '^[0-9A-Za-z.+-]+$') {
        throw "`"$version`" isn't a version steamcards has."
    }

    # Which steamcards is installed already, if any: the one in the install
    # folder, or else the first one on PATH.
    $current = Join-Path $installDir 'steamcards.exe'
    if (-not (Test-Path -LiteralPath $current)) {
        $found = Get-Command steamcards.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        $current = if ($found) { $found.Path } else { $null }
    }
    $dir = if ($current) { Split-Path -Parent $current } else { $installDir }
    $exe = Join-Path $dir 'steamcards.exe'

    # Its version, from "steamcards 0.1.0". Builds from before --version
    # existed fail here, and are replaced.
    $installed = $null
    $order = $null
    if ($current) {
        try {
            $out = & $current --version 2>$null
            if ("$out" -match '^steamcards (\S+)') { $installed = $Matches[1] }
        } catch {
            $installed = $null
        }
    }
    if ($installed) {
        $order = Compare-Version $installed $version
        if ($env:STEAMCARDS_FORCE -ne '1') {
            if ($order -eq 0) {
                Write-Host "steamcards $installed is already up to date."
                return
            }
            if ($order -gt 0 -and -not $env:STEAMCARDS_VERSION) {
                Write-Host "steamcards $installed is newer than the latest release, $version. Leaving it as it is."
                return
            }
        }
    }

    # A copy that cargo built is cargo's to update.
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    if ($current -and $dir -eq (Join-Path $cargoHome 'bin')) {
        throw "$current was built from source with cargo. Update it the same way: https://github.com/$repo/blob/main/CONTRIBUTING.md#building-from-source"
    }

    # A running steamcards.exe can't be replaced.
    $running = Get-Process steamcards -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
    if ($running) {
        throw 'steamcards is running. Close it, then run this command again.'
    }

    # The folder has to be one you can write to: this script never asks for
    # administrator rights. Found out now, before anything is downloaded.
    if (Test-Path -LiteralPath $dir) {
        $probe = Join-Path $dir ".steamcards-write-test-$([guid]::NewGuid())"
        try {
            [IO.File]::WriteAllText($probe, '')
            Remove-Item -LiteralPath $probe -Force
        } catch {
            if ($current) {
                throw "steamcards is in $dir, which only an administrator can change. Update it the way it was installed, or delete it and run this command again to install it to $installDir."
            }
            throw "Can't write to $dir, and this script doesn't use administrator rights."
        }
    }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) "steamcards-$([guid]::NewGuid())"
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $base = "$releases/download/v$version"
        $zip = Join-Path $tmp $asset
        $sums = Join-Path $tmp 'SHA256SUMS'
        Write-Host "Downloading steamcards $version..."
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $zip
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums
        } catch {
            throw "Couldn't download steamcards $version from $base ($($_.Exception.Message))"
        }

        $line = Get-Content -LiteralPath $sums | Where-Object { ($_ -split '\s+')[1] -eq $asset } | Select-Object -First 1
        if (-not $line) { throw "SHA256SUMS doesn't list $asset." }
        $expected = ($line -split '\s+')[0]
        $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash
        if ($actual -ne $expected) {
            throw "$asset doesn't match its checksum in SHA256SUMS, so nothing was installed."
        }

        $unpacked = Join-Path $tmp 'unpacked'
        Expand-Archive -LiteralPath $zip -DestinationPath $unpacked

        # Copied in beside the old one under a temporary name, then renamed
        # over it.
        $new = Join-Path $dir 'steamcards.exe.new'
        try {
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
            Copy-Item -LiteralPath (Join-Path $unpacked 'steamcards.exe') -Destination $new -Force
        } catch {
            throw "Couldn't write to $dir ($($_.Exception.Message))"
        }
        try {
            Move-Item -LiteralPath $new -Destination $exe -Force
        } catch {
            Remove-Item -LiteralPath $new -Force -ErrorAction SilentlyContinue
            throw "Couldn't replace $exe. If steamcards is running, close it, then run this command again."
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    # The install folder goes on the user PATH, if it isn't there yet. The
    # value is read and written unexpanded, so entries like %USERPROFILE%\bin
    # stay as they are.
    $pathChanged = $false
    if ($dir -eq $installDir) {
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            $userPath = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $entries = @($userPath.Split(';') | Where-Object { $_ })
            if ($entries -notcontains $installDir) {
                $newPath = ($entries + $installDir) -join ';'
                $key.SetValue('Path', $newPath, [Microsoft.Win32.RegistryValueKind]::ExpandString)
                $pathChanged = $true
            }
        } finally {
            $key.Close()
        }
        if ($pathChanged) {
            # Setting a variable through .NET tells Windows the environment
            # changed, so terminals opened from now on see the new PATH.
            [Environment]::SetEnvironmentVariable('STEAMCARDS_INSTALLING', '1', 'User')
            [Environment]::SetEnvironmentVariable('STEAMCARDS_INSTALLING', $null, 'User')
        }
    }

    $arrow = [char] 0x2192
    if (-not $current) {
        Write-Host "Installed steamcards $version to $exe."
    } elseif (-not $installed) {
        Write-Host "Updated steamcards to $version."
    } elseif ($order -lt 0) {
        Write-Host "Updated steamcards $installed $arrow $version."
    } elseif ($order -gt 0) {
        Write-Host "Downgraded steamcards $installed $arrow $version."
    } else {
        Write-Host "Reinstalled steamcards $version."
    }
    if ($pathChanged) {
        Write-Host "Added $installDir to your PATH. Restart your terminal, then run: steamcards"
    } elseif (-not $current) {
        Write-Host 'Run it with: steamcards'
    }
}
