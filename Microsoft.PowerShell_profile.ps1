oh-my-posh init pwsh --config $HOME\.oh-my-posh.omp.json | Invoke-Expression

$env:VIRTUAL_ENV_DISABLE_PROMPT = 1
$env:SHELL = "C:\Program Files\PowerShell\7\pwsh.exe"

# Make winget-installed tools win over machine-PATH copies of the same exe.
# Windows builds PATH as Machine-then-User, and winget shims live in the User
# PATH (...\WinGet\Links) -- so e.g. Docker Desktop's bundled kubectl (Machine
# PATH) would otherwise shadow the newer winget kubectl. Prepending the winget
# Links dir here, at shell startup, flips that for every interactive session.
$env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Links;$env:PATH"

# Make `bash`/`sh` resolve to Git Bash instead of the WSL launcher in
# System32\bash.exe. System32 sits early in the Machine PATH, so we prepend
# Git's bin at session start to win. NOTE: only Git\bin (has bash, sh, git) --
# NOT Git\usr\bin, which would shadow Windows tools like find/sort with the
# GNU coreutils versions.
$env:PATH = "C:\Program Files\Git\bin;$env:PATH"

# Import the Chocolatey Profile that contains the necessary code to enable
# tab-completions to function for `choco`.
# Be aware that if you are missing these lines from your profile, tab completion
# for `choco` will not function.
# See https://ch0.co/tab-completion for details.
$ChocolateyProfile = "$env:ChocolateyInstall\helpers\chocolateyProfile.psm1"
if (Test-Path($ChocolateyProfile)) {
  Import-Module "$ChocolateyProfile"
}

# Notebook support for Zed
$env:LOCAL_NOTEBOOK_DEV = 1

(& uv generate-shell-completion powershell) | Out-String | Invoke-Expression
(& uv generate-shell-completion powershell) | Out-String | Invoke-Expression

function todos { zed 'C:\Users\jherng\Documents\Obsidian\Daily Vault' }

# Enable kubectl autocompletion
if (Get-Command kubectl -ErrorAction SilentlyContinue) {
    kubectl completion powershell | Out-String | Invoke-Expression
}

# Create a short 'k' alias with working autocompletion
Set-Alias -Name k -Value kubectl
Register-ArgumentCompleter -Native -CommandName k -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    kubectl completion powershell | Out-String | Invoke-Expression
}