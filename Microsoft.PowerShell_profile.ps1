oh-my-posh init pwsh --config $HOME\.oh-my-posh.omp.json | Invoke-Expression

$env:VIRTUAL_ENV_DISABLE_PROMPT = 1
$env:SHELL = "C:\Program Files\PowerShell\7\pwsh.exe"

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

function todos { zed 'C:\Users\jherng\Documents\Obsidian Vault\TODOs' }
