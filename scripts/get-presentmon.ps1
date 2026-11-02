<#
  Baixa o PresentMon (console) da release oficial da Intel e confere o hash SHA-256 publicado no GitHub.
  O Nexus Optimizer usa o PresentMon para medir FPS no Benchmark. Licença do PresentMon: MIT (Intel).
  Uso:  powershell -ExecutionPolicy Bypass -File scripts/get-presentmon.ps1
#>
$ErrorActionPreference = 'Stop'
$tag  = 'v2.6.0'
$name = 'PresentMon-2.6.0-x64.exe'
$dest = Join-Path $PSScriptRoot '..\src-tauri\resources'
New-Item -ItemType Directory -Force $dest | Out-Null
$file = Join-Path $dest $name

$release = Invoke-RestMethod "https://api.github.com/repos/GameTechDev/PresentMon/releases/tags/$tag" -Headers @{ 'User-Agent' = 'nexus-optimizer' }
$asset   = $release.assets | Where-Object { $_.name -eq $name }
if (-not $asset) { throw "Arquivo $name não encontrado na release $tag." }

Invoke-WebRequest $asset.browser_download_url -OutFile $file
$hash = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()
$expected = ($asset.digest -replace '^sha256:', '').ToLower()
if ($expected -and $hash -ne $expected) {
    Remove-Item $file -Force
    throw "Hash diferente do publicado ($hash != $expected). Arquivo removido."
}
Write-Host "OK: $file ($hash)"
