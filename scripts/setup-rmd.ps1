<#
.SYNOPSIS
  Installs the `rmd` command (open .md files in Rusty Notepad from any shell).

.DESCRIPTION
  Copies scripts/rmd.cmd next to rust_notepad.exe and adds that folder to the
  *user* PATH, so `rmd notes.md` works from cmd/PowerShell anywhere.
  With -AssociateMd it also registers .md files (current user only, no admin)
  so double-clicking a Markdown file opens it in Rusty Notepad.
  With -Uninstall everything above is removed again.

.EXAMPLE
  .\scripts\setup-rmd.ps1
  .\scripts\setup-rmd.ps1 -ExePath 'C:\Program Files\Rusty Notepad\rust_notepad.exe' -AssociateMd
  .\scripts\setup-rmd.ps1 -Uninstall
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$ExePath = "",
    [switch]$AssociateMd,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'
$ProgId = 'RustyNotepad.Document'

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

function Find-Exe([string]$hint) {
    if ($hint -ne '' ) {
        if (-not (Test-Path $hint)) { throw "Executable not found: $hint" }
        return (Resolve-Path $hint).Path
    }
    $root = Get-RepoRoot
    foreach ($candidate in @('target\release\rust_notepad.exe', 'target\debug\rust_notepad.exe')) {
        $full = Join-Path $root $candidate
        if (Test-Path $full) { return (Resolve-Path $full).Path }
    }
    throw 'rust_notepad.exe not found. Build first (cargo build) or pass -ExePath.'
}

function Update-UserPath([string]$dir, [bool]$remove) {
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @($current -split ';' | Where-Object { $_ -ne '' })
    $already = $parts | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') }
    if ($remove) {
        if ($already) {
            $parts = @($parts | Where-Object { $_.TrimEnd('\') -ine $dir.TrimEnd('\') })
            if ($PSCmdlet.ShouldProcess("user PATH", "remove $dir")) {
                [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
            }
            Write-Host "Removed from user PATH: $dir"
        } else {
            Write-Host 'Folder was not on user PATH; nothing to remove.'
        }
        return
    }
    if ($already) {
        Write-Host 'Folder already on user PATH; nothing to do.'
        return
    }
    if ($PSCmdlet.ShouldProcess("user PATH", "add $dir")) {
        $newPath = (($parts + @($dir)) -join ';')
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    }
    Write-Host "Added to user PATH: $dir (restart your terminal to pick it up)"
}

function Broadcast-EnvChange {
    try {
        $sig = '[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'
        $t = Add-Type -MemberDefinition $sig -Name Win32EnvNotify -Namespace RmdSetup -PassThru
        $r = [UIntPtr]::Zero
        [void]$t::SendMessageTimeout([IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$r)
    } catch {
        Write-Warning "Could not broadcast environment change: $_"
    }
}

function Install-Association([string]$exe) {
    $mdKey = 'HKCU:\Software\Classes\.md'
    $progKey = "HKCU:\Software\Classes\$ProgId\shell\open\command"
    if ($PSCmdlet.ShouldProcess('.md association', "point at $exe")) {
        if (-not (Test-Path $mdKey)) { New-Item $mdKey -Force | Out-Null }
        Set-ItemProperty $mdKey -Name '(Default)' -Value $ProgId
        if (-not (Test-Path $progKey)) { New-Item $progKey -Force | Out-Null }
        Set-ItemProperty $progKey -Name '(Default)' -Value "`"$exe`" `"%1`""
        Set-ItemProperty "HKCU:\Software\Classes\$ProgId" -Name 'FriendlyAppName' -Value 'Rusty Notepad' -ErrorAction SilentlyContinue
    }
    Write-Host '.md files (current user) now open with Rusty Notepad.'
}

function Remove-Association {
    $mdKey = 'HKCU:\Software\Classes\.md'
    $progKey = "HKCU:\Software\Classes\$ProgId"
    if ($PSCmdlet.ShouldProcess('.md association', 'remove Rusty Notepad')) {
        $cur = (Get-ItemProperty $mdKey -Name '(Default)' -ErrorAction SilentlyContinue).'(Default)'
        if ($cur -eq $ProgId -and (Test-Path $mdKey)) {
            Clear-ItemProperty $mdKey -Name '(Default)' -ErrorAction SilentlyContinue
        }
        if (Test-Path $progKey) { Remove-Item $progKey -Recurse -Force }
    }
    Write-Host 'Removed Rusty Notepad .md association (if it was ours).'
}

$exe = Find-Exe $ExePath
$dir = Split-Path $exe -Parent
$shim = Join-Path $dir 'rmd.cmd'

if ($Uninstall) {
    if ($PSCmdlet.ShouldProcess($shim, 'delete rmd.cmd shim')) {
        Remove-Item $shim -Force -ErrorAction SilentlyContinue
    }
    Write-Host 'Removed rmd.cmd shim (if present).'
    Update-UserPath $dir $true
    Remove-Association
    Broadcast-EnvChange
    return
}

$source = Join-Path (Get-RepoRoot) 'scripts\rmd.cmd'
if (-not (Test-Path $source)) { throw "Missing shim template: $source" }
if ($PSCmdlet.ShouldProcess($shim, 'install rmd.cmd shim')) {
    Copy-Item $source $shim -Force
}
Write-Host "Installed shim: $shim"
Update-UserPath $dir $false
if ($AssociateMd) { Install-Association $exe }
Broadcast-EnvChange
Write-Host ''
Write-Host 'Done. Open a NEW terminal and try:  rmd notes.md'
