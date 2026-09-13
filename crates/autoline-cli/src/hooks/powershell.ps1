# Autoline PowerShell Integration Hook
# Automatically queries the autoline daemon for inline ghost-text completions
# and records command executions into history.

if ($host.Name -eq 'ConsoleHost') {
    if (Get-Module -ListAvailable -Name PSReadLine) {
        Import-Module PSReadLine -ErrorAction SilentlyContinue

        # Tab: fetch ghost-text suggestion from autoline and insert it.
        # Falls back to normal tab completion when no suggestion is available.
        Set-PSReadLineKeyHandler -Key Tab -ScriptBlock {
            param($key, $arg)
            $line = $null
            $cursor = $null
            [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)

            if ($line -and $cursor -eq $line.Length) {
                $cwd = (Get-Location).Path
                $ghost = autoline suggest --line "$line" --cwd "$cwd" 2>$null
                if ($ghost) {
                    [Microsoft.PowerShell.PSConsoleReadLine]::Insert($ghost)
                    return
                }
            }
            [Microsoft.PowerShell.PSConsoleReadLine]::TabCompleteNext($key, $arg)
        }
    }
}

# Record the last executed command into autoline history.
# Filters out autoline meta-commands so they don't pollute suggestion history.
function Global:Autoline-PostExecute {
    param($Line)
    if (-not $Line -or $Line.Trim() -eq '') { return }

    # Don't record autoline/autolined commands themselves — they would
    # pollute the suggestion engine with "powershell", "--print-hook", etc.
    $trimmed = $Line.Trim()
    if ($trimmed -match '^(autoline|autolined)\b') { return }

    $cwd = (Get-Location).Path
    autoline record --line "$Line" --shell powershell --cwd "$cwd" 2>&1 | Out-Null
}

# Wrap the existing prompt to call Autoline-PostExecute after each command.
if (-not (Test-Path Variable:Global:__AutolineOriginalPrompt)) {
    $Global:__AutolineOriginalPrompt = $function:prompt
    $lastOnLoad = Get-History -Count 1 -ErrorAction SilentlyContinue
    $Global:__AutolineLastId = if ($lastOnLoad) { $lastOnLoad.Id } else { 0 }

    function Global:prompt {
        $lastHistory = Get-History -Count 1 -ErrorAction SilentlyContinue
        if ($lastHistory -and ($Global:__AutolineLastId -ne $lastHistory.Id)) {
            $Global:__AutolineLastId = $lastHistory.Id
            Autoline-PostExecute -Line $lastHistory.CommandLine
        }
        if ($Global:__AutolineOriginalPrompt) {
            & $Global:__AutolineOriginalPrompt
        } else {
            "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
        }
    }
}
