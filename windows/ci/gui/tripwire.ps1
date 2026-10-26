# 실제 사용자 환경을 건드리지 않았는지 지키는 철선 — GUI 시험 앞뒤로 돈다(windows/ci/gui/README.md).
# 지키는 것: (1) 사용자 settings.json 과 PowerShell profile.ps1 의 md5(해시만 기록한다. 내용은 읽어 출력하지 않는다),
#            (2) 격리한 보드 밑에서 진짜 claude.exe 가 돌지 않는가.
# PowerShell 5.1 과 7 모두에서 돈다.
#
#   pwsh -File tripwire.ps1 -Phase before   -State <상태.json> [-BoardPid <pid>] [-FakeDir <가짜 claude 폴더>]
#   pwsh -File tripwire.ps1 -Phase after    -State <상태.json>
#   pwsh -File tripwire.ps1 -Phase check    -State <상태.json>        (해시는 보지 않고 진짜 claude 만 본다 — 시험 도중에 언제든)
#   pwsh -File tripwire.ps1 -Phase selftest                            (임시 사본으로 변화 감지를 증명한다. 실제 파일은 읽지도 않는다)
#
# 인자
#   -State       상태 파일(JSON). before 가 쓰고 after·check 가 읽는다. 지키는 파일 안에 두지 않는다
#   -BoardPid    격리한 보드(ClaudeBoard.exe)의 pid. before 에서 정하면 상태에 남고, 이후 단계가 다시 받아도 된다
#   -FakeDir     가짜 claude(node 사본 claude.exe)가 든 폴더. 이 아래 claude.exe 는 허용한다
#   -SettingsPath / -ProfilePaths   지킬 파일(기본: ~\.claude\settings.json, 문서\PowerShell\profile.ps1, 문서\WindowsPowerShell\profile.ps1)
#
# 판정 — 종료코드가 답이다(출력을 파이프에 물리지 않는다)
#   0  바뀐 것 없음, 보드 밑에 진짜 claude 없음
#   1  지킨 파일의 md5 가 바뀌었거나, 격리 보드의 자손 중 허용 폴더 밖 claude.exe 가 돈다
#   2  사용법 오류(상태 파일 없음 등)
# 출력 줄은 `TRIPWIRE|<종류>|…` 로 시작한다. OTHER-CLAUDE 는 보드와 무관한 다른 세션의 claude.exe 를 알리는 참고 줄이고 실패가 아니다.
param(
  [Parameter(Mandatory = $true)][ValidateSet('before', 'after', 'check', 'selftest')][string]$Phase,
  [string]$State,
  [int]$BoardPid = 0,
  [string]$FakeDir,
  [string]$SettingsPath,
  [string[]]$ProfilePaths
)

$ErrorActionPreference = 'Stop'
# 한글 출력이 부르는 쪽(node·CI)에서 UTF-8 로 읽히게 한다
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)

function Get-DefaultWatch {
  $docs = [Environment]::GetFolderPath('MyDocuments')
  $settings = $SettingsPath
  if (-not $settings) { $settings = Join-Path (Join-Path $env:USERPROFILE '.claude') 'settings.json' }
  $profiles = $ProfilePaths
  if (-not $profiles) {
    $profiles = @((Join-Path $docs 'PowerShell\profile.ps1'), (Join-Path $docs 'WindowsPowerShell\profile.ps1'))
  }
  $list = @([pscustomobject]@{ label = 'settings.json'; path = $settings })
  foreach ($p in $profiles) { $list += [pscustomobject]@{ label = 'profile:' + (Split-Path -Leaf (Split-Path -Parent $p)); path = $p } }
  return $list
}

# 파일 하나의 md5 — 없으면 ABSENT. 내용은 해시 계산 말고는 읽지 않는다
function Get-Md5([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return 'ABSENT' }
  # 모듈 없이 .NET 으로 계산한다(PSModulePath 가 어긋난 셸에서도 도는 것을 우선한다). 다른 프로그램이 쥐고 있어도 읽을 수 있게 공유 읽기로 연다
  $md5 = [Security.Cryptography.MD5]::Create()
  $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
  try { $bytes = $md5.ComputeHash($stream) } finally { $stream.Dispose(); $md5.Dispose() }
  return ([BitConverter]::ToString($bytes) -replace '-', '').ToLowerInvariant()
}

function Get-ProcessTable {
  $table = @{}
  foreach ($p in (Get-CimInstance Win32_Process)) {
    $table[[int]$p.ProcessId] = [pscustomobject]@{ pid = [int]$p.ProcessId; ppid = [int]$p.ParentProcessId; name = $p.Name; path = $p.ExecutablePath; created = $p.CreationDate }
  }
  return $table
}

# pid 의 조상 사슬 — 부모가 자식보다 늦게 만들어졌으면 pid 재사용이라 끊는다
function Get-Ancestors($Table, [int]$ProcessId) {
  $chain = @()
  $seen = @{}
  $node = $Table[$ProcessId]
  while ($node -and -not $seen.ContainsKey($node.pid)) {
    $seen[$node.pid] = $true
    $parent = $Table[$node.ppid]
    if (-not $parent) { break }
    if ($parent.created -and $node.created -and $parent.created -gt $node.created) { break }
    $chain += $parent.pid
    $node = $parent
  }
  return $chain
}

function Test-UnderDir([string]$Path, [string]$Dir) {
  if (-not $Path -or -not $Dir) { return $false }
  $root = [IO.Path]::GetFullPath($Dir).TrimEnd('\') + '\'
  return [IO.Path]::GetFullPath($Path).StartsWith($root, [StringComparison]::OrdinalIgnoreCase)
}

# claude.exe 를 가른다 — 보드 자손이면서 허용 폴더 밖이면 진짜(real), 보드와 무관하면 참고(other)
function Find-Claude([int]$Board, [string]$Fake) {
  $table = Get-ProcessTable
  $real = @()
  $other = @()
  foreach ($p in $table.Values) {
    if ($p.name -ne 'claude.exe') { continue }
    if ($p.path -and (Test-UnderDir $p.path $Fake)) { continue }
    $chain = Get-Ancestors $table $p.pid
    if ($Board -gt 0 -and ($chain -contains $Board)) { $real += $p } else { $other += $p }
  }
  return [pscustomobject]@{ real = $real; other = $other }
}

function Write-ClaudeReport($Found) {
  foreach ($p in $Found.other) { Write-Output ("TRIPWIRE|OTHER-CLAUDE|{0}|{1}" -f $p.pid, $p.path) }
  foreach ($p in $Found.real) { Write-Output ("TRIPWIRE|REAL-CLAUDE|{0}|{1}" -f $p.pid, $p.path) }
}

function Read-State {
  if (-not $State -or -not (Test-Path -LiteralPath $State -PathType Leaf)) {
    Write-Output 'TRIPWIRE|USAGE|상태 파일이 없다 — 먼저 -Phase before -State <파일> 을 돌린다'
    exit 2
  }
  return (Get-Content -LiteralPath $State -Raw -Encoding UTF8 | ConvertFrom-Json)
}

function Invoke-Phase([string]$Which) {
  if ($Which -eq 'before') {
    if (-not $State) { Write-Output 'TRIPWIRE|USAGE|-State 가 필요하다'; exit 2 }
    $watch = @()
    foreach ($w in (Get-DefaultWatch)) {
      if (Test-UnderDir $State ([IO.Path]::GetDirectoryName($w.path))) { Write-Output ('TRIPWIRE|USAGE|상태 파일을 지키는 파일의 폴더 안에 두지 않는다: ' + $State); exit 2 }
      $watch += [pscustomobject]@{ label = $w.label; path = $w.path; md5 = (Get-Md5 $w.path) }
    }
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($State))) | Out-Null
    $doc = [pscustomobject]@{ takenAt = (Get-Date).ToString('o'); boardPid = $BoardPid; fakeDir = $FakeDir; watch = $watch }
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($State), ($doc | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
    foreach ($w in $watch) { Write-Output ('TRIPWIRE|RECORDED|{0}|{1}' -f $w.label, $w.md5) }
    $found = Find-Claude $BoardPid $FakeDir
    Write-ClaudeReport $found
    if ($found.real.Count -gt 0) { Write-Output 'TRIPWIRE|FAIL|보드 밑에 진짜 claude 가 이미 돈다'; exit 1 }
    Write-Output 'TRIPWIRE|OK|before'
    exit 0
  }

  $saved = Read-State
  $board = $BoardPid
  if ($board -le 0) { $board = [int]$saved.boardPid }
  $fake = $FakeDir
  if (-not $fake) { $fake = [string]$saved.fakeDir }
  $failed = $false

  if ($Which -eq 'after') {
    foreach ($w in $saved.watch) {
      $now = Get-Md5 $w.path
      if ($now -ne $w.md5) {
        Write-Output ('TRIPWIRE|CHANGED|{0}|{1}|{2}' -f $w.label, $w.md5, $now)
        $failed = $true
      } else {
        Write-Output ('TRIPWIRE|SAME|{0}|{1}' -f $w.label, $now)
      }
    }
  }
  $found = Find-Claude $board $fake
  Write-ClaudeReport $found
  if ($found.real.Count -gt 0) { $failed = $true }
  if ($failed) { Write-Output ('TRIPWIRE|FAIL|' + $Which); exit 1 }
  Write-Output ('TRIPWIRE|OK|' + $Which)
  exit 0
}

if ($Phase -ne 'selftest') { Invoke-Phase $Phase }

# ── selftest: 임시 폴더의 사본만 쓴다. 실제 settings.json·profile.ps1 은 읽지도 않는다 ─────────────────────
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('tripwire-selftest-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
  $settings = Join-Path $tmp 'home\.claude\settings.json'
  $profileFile = Join-Path $tmp 'docs\PowerShell\profile.ps1'
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $settings), (Split-Path -Parent $profileFile) | Out-Null
  [IO.File]::WriteAllText($settings, '{"selftest":1}')
  [IO.File]::WriteAllText($profileFile, '# selftest profile')
  $stateFile = Join-Path $tmp 'state\state.json'
  $self = $MyInvocation.MyCommand.Path
  $shell = (Get-Process -Id $PID).Path
  function Invoke-Self([string]$Which) {
    $out = & $shell -NoProfile -File $self -Phase $Which -State $stateFile -SettingsPath $settings -ProfilePaths $profileFile 2>&1
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($out -join "`n") }
  }
  $b = Invoke-Self 'before'
  $clean = Invoke-Self 'after'
  [IO.File]::WriteAllText($profileFile, '# selftest profile, changed')
  $changed = Invoke-Self 'after'
  $ok = ($b.code -eq 0) -and ($clean.code -eq 0) -and ($changed.code -eq 1) -and ($changed.text -match 'TRIPWIRE\|CHANGED\|profile:')
  foreach ($r in @($b, $clean, $changed)) { Write-Output $r.text }
  if ($ok) { Write-Output 'TRIPWIRE|SELFTEST|PASS|임시 사본의 변화를 잡았고 바뀌지 않은 사본은 통과시켰다'; $code = 0 }
  else { Write-Output ('TRIPWIRE|SELFTEST|FAIL|before={0} clean={1} changed={2}' -f $b.code, $clean.code, $changed.code); $code = 1 }
} finally {
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
exit $code
