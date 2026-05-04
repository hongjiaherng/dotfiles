# dotfiles

Single source of truth for Windows configs. Consumer locations are symlinks back here.

## Tracked

| Repo path | Target |
|---|---|
| `.oh-my-posh.omp.json` | `$HOME\.oh-my-posh.omp.json` |
| `Microsoft.PowerShell_profile.ps1` | `$PROFILE` |
| `Microsoft.WindowsTerminal\settings.json` | `$env:LOCALAPPDATA\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json` |
| `vscode\settings.json` | `$env:APPDATA\Code\User\settings.json` |
| `cursor\settings.json` | `$env:APPDATA\Cursor\User\settings.json` |
| `zed\settings.json` | `$env:APPDATA\Zed\settings.json` |
| `zed\themes` | `$env:APPDATA\Zed\themes` |
| `nvim` | `$env:LOCALAPPDATA\nvim` |
| `rainmeter\ObsidianDaily` | `$HOME\Documents\Rainmeter\Skins\ObsidianDaily` |

WT path is for the Store stable build. Preview: `Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe`. Unpackaged: `$env:LOCALAPPDATA\Microsoft\Windows Terminal\settings.json`.

## Setup on a new PC

1. Enable Developer Mode (Settings -> Privacy & security -> For developers), or run the script elevated.
2. Clone: `git clone <repo-url> $HOME\System\dotfiles`
3. Install: PowerShell 7, Oh My Posh, Windows Terminal, a Nerd Font (not the Mono variant), Neovim, VS Code, Cursor, Zed.
4. Run the script below from PowerShell 7.

```powershell
$repo = "$HOME\System\dotfiles"

function Link-Config($source, $target) {
    $parent = Split-Path $target -Parent
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    if (Test-Path $target) {
        $item = Get-Item $target -Force
        if ($item.LinkType -ne "SymbolicLink") { Move-Item $target "$target.bak" -Force }
        else { Remove-Item $target -Force -Recurse }
    }
    New-Item -ItemType SymbolicLink -Path $target -Target $source | Out-Null
}

Link-Config "$repo\.oh-my-posh.omp.json" "$HOME\.oh-my-posh.omp.json"
Link-Config "$repo\Microsoft.PowerShell_profile.ps1" $PROFILE
Link-Config "$repo\Microsoft.WindowsTerminal\settings.json" "$env:LOCALAPPDATA\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"
Link-Config "$repo\vscode\settings.json" "$env:APPDATA\Code\User\settings.json"
Link-Config "$repo\cursor\settings.json" "$env:APPDATA\Cursor\User\settings.json"
Link-Config "$repo\zed\settings.json" "$env:APPDATA\Zed\settings.json"
Link-Config "$repo\zed\themes" "$env:APPDATA\Zed\themes"
Link-Config "$repo\nvim" "$env:LOCALAPPDATA\nvim"
Link-Config "$repo\rainmeter\ObsidianDaily" "$HOME\Documents\Rainmeter\Skins\ObsidianDaily"
```

Verify: `Get-Item <path> | Select LinkType, Target`.

Extensions are not tracked.

## Adding a config

Move the file into the repo, add a `Link-Config` line, commit.
