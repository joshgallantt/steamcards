# Uninstalls steamcards from Windows.
#
#   irm https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/uninstall.ps1 | iex
#
# It finds steamcards as install.ps1 does, in
# %LOCALAPPDATA%\Programs\steamcards or else the first one on your PATH, and
# removes it the way it was installed: a copy that cargo built with cargo, and
# any other by deleting it (the whole folder, when it's that one). It takes
# that folder off your user PATH too, where install.ps1 put it. Then it asks
# whether to delete your sign-in and choices, in %LOCALAPPDATA%\steamcards,
# as well. Where it can't ask, it keeps them. To answer before it asks:
#
#   $env:STEAMCARDS_DELETE_DATA=1  delete your sign-in and choices, without asking
#   $env:STEAMCARDS_DELETE_DATA=0  keep them, without asking
#
# No admin rights needed.

# In a script block, so nothing it sets is left behind in your session.
& {
    $ErrorActionPreference = 'Stop'

    $installDir = Join-Path $env:LOCALAPPDATA 'Programs\steamcards'
    # Your sign-in, choices and saved prices are in the folder steamcards
    # keeps them in: the one Rust's dirs crate gives it for this computer's
    # settings (dirs::config_local_dir, which is %LOCALAPPDATA%), plus
    # steamcards. It's the only folder of yours this ever deletes.
    $data = Join-Path $env:LOCALAPPDATA 'steamcards'

    # Which steamcards is installed, if any: the one in the install folder,
    # or else the first one on PATH.
    $current = Join-Path $installDir 'steamcards.exe'
    if (-not (Test-Path -LiteralPath $current)) {
        $found = Get-Command steamcards.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        $current = if ($found) { $found.Path } else { $null }
    }

    if ($current) {
        $dir = Split-Path -Parent $current

        # A running steamcards.exe can't be deleted.
        $running = Get-Process steamcards -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $current }
        if ($running) {
            throw 'steamcards is running. Close it, then run this command again.'
        }

        $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
        if ($dir -eq (Join-Path $cargoHome 'bin')) {
            # A copy that cargo built is cargo's to remove, so that it forgets
            # it too.
            $cargo = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
            if (-not $cargo) {
                throw "steamcards was built from source with cargo, but cargo isn't on your PATH. Uninstall it with: cargo uninstall steamcards"
            }
            Write-Host 'steamcards was built from source with cargo. Uninstalling it with: cargo uninstall steamcards'
            # Only cargo's exit code counts: Windows PowerShell 5.1 can take
            # what cargo writes to its error output for errors.
            $ErrorActionPreference = 'Continue'
            try {
                & $cargo.Path uninstall steamcards
            } finally {
                $ErrorActionPreference = 'Stop'
            }
            if ($LASTEXITCODE -ne 0) {
                throw "cargo couldn't uninstall steamcards."
            }
        } else {
            # The install folder is steamcards' own, so it goes whole. A copy
            # anywhere else goes by itself.
            $target = if ($dir -eq $installDir) { $installDir } else { $current }
            Write-Host "Deleting $target."
            try {
                Remove-Item -LiteralPath $target -Recurse -Force
            } catch {
                throw "Couldn't delete $target ($($_.Exception.Message))"
            }
        }
    }

    # The install folder comes off the user PATH, where install.ps1 put it,
    # and nothing else does. The value is read and written unexpanded, as
    # install.ps1 does, so entries like %USERPROFILE%\bin stay as they are.
    # The user PATH is in the registry, so this is for Windows only.
    $pathChanged = $false
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            $userPath = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $entries = $userPath.Split(';')
            $kept = @($entries | Where-Object { $_ -ne $installDir })
            if ($kept.Count -lt $entries.Count) {
                $key.SetValue('Path', ($kept -join ';'), $key.GetValueKind('Path'))
                $pathChanged = $true
            }
        } finally {
            $key.Close()
        }
        if ($pathChanged) {
            # Setting a variable through .NET tells Windows the environment
            # changed, so terminals opened from now on see the new PATH.
            [Environment]::SetEnvironmentVariable('STEAMCARDS_UNINSTALLING', '1', 'User')
            [Environment]::SetEnvironmentVariable('STEAMCARDS_UNINSTALLING', $null, 'User')
        }
    }
    # A folder left on the PATH, after steamcards was deleted by hand, is
    # the last of it to uninstall.
    $uninstalled = $current -or $pathChanged

    # Your sign-in and choices go too, if you say so.
    $dataLeft = 'none'
    if (Test-Path -LiteralPath $data -PathType Container) {
        if ($uninstalled) {
            $question = 'Also delete your sign-in and choices? [y/N]'
        } else {
            Write-Host "steamcards isn't installed."
            $question = 'Delete your sign-in and choices? [y/N]'
        }
        if ($env:STEAMCARDS_DELETE_DATA -eq '1') {
            $delete = $true
        } elseif ($env:STEAMCARDS_DELETE_DATA -eq '0') {
            $delete = $false
        } else {
            # Only someone at a terminal can answer. A session that can't
            # prompt (-NonInteractive, or with its input from a pipe or a
            # file) keeps them.
            $answer = $null
            if (-not [Console]::IsInputRedirected) {
                try {
                    $answer = Read-Host $question
                } catch {
                    $answer = $null
                }
            }
            $delete = "$answer".Trim() -match '^(y|yes)$'
        }
        if ($delete) {
            try {
                $folder = Get-Item -LiteralPath $data -Force
                if ($folder.Attributes.HasFlag([IO.FileAttributes]::ReparsePoint)) {
                    # A link to a folder somewhere else: only the link goes.
                    # (Windows PowerShell 5.1's Remove-Item would empty the
                    # folder it points to.)
                    $folder.Delete()
                } else {
                    Remove-Item -LiteralPath $data -Recurse -Force
                }
            } catch {
                throw "Couldn't delete $data ($($_.Exception.Message))"
            }
            $dataLeft = 'deleted'
        } else {
            $dataLeft = 'kept'
        }
    }

    if ($uninstalled) {
        if ($dataLeft -eq 'deleted') {
            Write-Host 'Uninstalled steamcards and deleted your sign-in and choices.'
        } elseif ($dataLeft -eq 'kept') {
            Write-Host "Uninstalled steamcards. Your sign-in and choices are still in $data."
        } else {
            Write-Host 'Uninstalled steamcards.'
        }
    } elseif ($dataLeft -eq 'deleted') {
        Write-Host 'Deleted your sign-in and choices.'
    } elseif ($dataLeft -eq 'kept') {
        Write-Host "Nothing was removed. Your sign-in and choices are still in $data."
    } else {
        Write-Host "steamcards isn't installed, so there's nothing to remove."
    }
}
