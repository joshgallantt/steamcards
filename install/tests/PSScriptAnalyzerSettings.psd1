# PSScriptAnalyzer's settings for install.ps1 and its tests:
#
#   Invoke-ScriptAnalyzer -Path install/install.ps1 -Settings install/tests/PSScriptAnalyzerSettings.psd1
@{
    Severity = @('Error', 'Warning')

    # Both scripts talk to the person running them, and Write-Host is how a
    # script does that. Write-Output would make the messages part of the
    # script's output instead.
    ExcludeRules = @('PSAvoidUsingWriteHost')

    Rules = @{
        # install.ps1 has to work in Windows PowerShell 5.1 as well as 7.
        PSUseCompatibleSyntax = @{
            Enable = $true
            TargetVersions = @('5.1', '7.0')
        }
    }
}
