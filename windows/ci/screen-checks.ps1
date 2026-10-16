# Claude Board for Windows — 실제 Windows 러너의 화면에서 보드·단일 실행·알림 패널을 확인한다 (.github/workflows/windows.yml 의 smoke 작업)
#
#   pwsh windows/ci/screen-checks.ps1 -Assets <claude-board-windows 아티팩트 폴더> -Results <smoke-results 폴더>
#
# 정보용이다(워크플로에서 continue-on-error) — 화면 세션·WebView2 렌더링은 러너마다 다를 수 있다.
# setup.exe /S 로 깐 상태를 다시 만들고, 살아 있는 가짜 세션(작업 중·승인 대기)을 둔 뒤:
#   보드 창이 뜨는지·스크린샷, 두 번째 실행이 곧 끝나고 첫 보드가 남는지(단일 실행), 트레이는 스크린샷으로만,
#   등록된 notify hook 을 가짜 Claude 아래에서 돌려 알림 패널이 뜨는지 — 포커스를 가져가지 않는지(GetForegroundWindow),
#   확장 스타일 WS_EX_NOACTIVATE·WS_EX_TOOLWINDOW·WS_EX_TOPMOST, 스스로 닫히는지, 스크린샷.
# 결과는 항목·기대·실제·통과 표로 남기고, 하나라도 실패하면 끝에 throw 한다(단계가 실패로 표시되지만 작업은 계속된다).
param(
  [Parameter(Mandatory = $true)][string]$Assets,
  [Parameter(Mandatory = $true)][string]$Results
)

. (Join-Path $PSScriptRoot 'common.ps1')
$CbCurrent.Annotate = 'warning'
Start-Transcript -Path (Join-Path $Results 'screen-checks.log') -Force | Out-Null
Initialize-Win32
$app = Join-Path $CbSetupDir 'ClaudeBoard.exe'

# 등록된 notify hook 을 돌려 새 알림 패널이 뜨는지 본다 — 패널이 뜨기 전후의 포그라운드 창, 확장 스타일, 스크린샷
function Test-NotifyPanel {
  param([string]$Label, [string]$EventName, [string]$SessionId, [string]$Cwd, [hashtable]$Extra, [string]$Shot)
  [void][CbWin]::SetCursorPos(1, 1)   # 마우스가 패널 위에 있으면 카운트다운이 멈춘다
  $fgBefore = [CbWin]::Foreground()
  Write-Host "  포그라운드 창(전): $(Format-Window $fgBefore)"
  $before = @(Get-BoardProcesses | ForEach-Object { $_.Pid })
  $res = Send-HookEvent $CbState.Fake $EventName $SessionId $Cwd $Extra
  $notifyRun = @($res.results | Where-Object { @($_.args)[0] -eq 'notify' }) | Select-Object -First 1
  if ($notifyRun) {
    Add-Check -Item "${Label}: notify hook 이 패널을 기다리지 않고 끝남" -Expected '종료 0 · 오류 없음 · 5000ms 안' -Actual ("종료 {0} · 오류 {1} · {2}ms" -f (Format-Cell $notifyRun.status), (Format-Cell $notifyRun.error), $notifyRun.ms) -Pass (($notifyRun.status -eq 0) -and -not $notifyRun.error -and $notifyRun.ms -lt 5000)
  } else {
    Add-Check -Item "${Label}: settings.json 에 notify hook 이 있음" -Expected '있음' -Actual '없음' -Pass $false
  }
  $panel = Wait-Until -TimeoutSec 20 -IntervalMs 100 -Condition {
    foreach ($q in @(Get-BoardProcesses)) {
      if ($before -contains $q.Pid -or "$($q.CommandLine)" -notmatch '--notify') { continue }
      $hs = [CbWin]::VisibleWindows([uint32]$q.Pid)
      if ($hs.Length -gt 0) { return [pscustomobject]@{ Pid = $q.Pid; Hwnd = $hs[0]; All = $hs; CommandLine = $q.CommandLine } }
    }
  }
  $spawned = @(Get-BoardProcesses | Where-Object { $before -notcontains $_.Pid -and "$($_.CommandLine)" -match '--notify' })
  Test-Value "${Label}: 알림 패널 프로세스(ClaudeBoard.exe --notify)가 뜸" $true ($spawned.Count -ge 1 -or $null -ne $panel)
  Test-Value "${Label}: 패널 창이 20초 안에 보임" $true ($null -ne $panel)
  if ($null -eq $panel) {
    foreach ($s in $spawned) { Write-Host "    패널 프로세스 $($s.Pid): $($s.CommandLine)" }
    Show-File (Join-Path $CbNotifier 'board.log') -Tail 80
    $null = Save-Screenshot $Shot
    return $null
  }
  Write-Host "  패널: pid $($panel.Pid) — $($panel.CommandLine)"
  Start-Sleep -Milliseconds 700   # 0.28초 올라오는 애니메이션이 끝난 뒤
  $fgAfter = [CbWin]::Foreground()
  $ex = [CbWin]::ExStyle($panel.Hwnd)
  Test-Value "${Label}: 패널 창" '' (@($panel.All | ForEach-Object { Format-Window $_ }) -join ' / ') -Info
  Add-Check -Item "${Label}: 포커스를 가져가지 않음 — GetForegroundWindow 가 그대로" -Expected (Format-Window $fgBefore) -Actual (Format-Window $fgAfter) -Pass ($fgAfter -eq $fgBefore)
  Add-Check -Item "${Label}: 포그라운드 창이 패널이 아님" -Expected "패널(0x$('{0:X}' -f $panel.Hwnd)) 아님" -Actual (Format-Window $fgAfter) -Pass ($fgAfter -ne $panel.Hwnd)
  foreach ($bit in @(@('WS_EX_NOACTIVATE', 0x08000000), @('WS_EX_TOOLWINDOW', 0x80), @('WS_EX_TOPMOST', 0x8))) {
    Add-Check -Item "${Label}: 확장 스타일 $($bit[0])" -Expected '켜짐' -Actual ('0x{0:X8}' -f $ex) -Pass (($ex -band $bit[1]) -ne 0)
  }
  Add-Check -Item "${Label}: WS_EX_APPWINDOW 없음(작업 표시줄에 나오지 않음)" -Expected '꺼짐' -Actual ('0x{0:X8}' -f $ex) -Pass (($ex -band 0x40000) -eq 0)
  $null = Save-Screenshot $Shot
  return $panel
}

try {
  Invoke-Scenario -Name '화면 1) 준비 — setup.exe /S, 살아 있는 가짜 세션(작업 중·승인 대기)' -Body {
    Reset-Machine
    $code = Install-WithSetup
    Test-Value 'setup.exe /S 종료 코드' 0 $code
    Stop-ClaudeBoardProcesses
    $projA = New-Project 'demo-proj'
    $projB = New-Project 'api-server'
    $fake = Start-FakeClaude -WorkDir $projA
    $CbState.Fake = $fake
    $CbState.ProjA = $projA
    $CbState.ProjB = $projB
    $CbState.SidA = 'ci-screen-a-' + [guid]::NewGuid().ToString('N').Substring(0, 6)
    $CbState.SidB = 'ci-screen-b-' + [guid]::NewGuid().ToString('N').Substring(0, 6)
    $a = $CbState.SidA
    $b = $CbState.SidB
    $null = Send-HookEvent $fake 'SessionStart' $a $projA @{ source = 'startup'; model = 'claude-opus-5-5' }
    $null = Send-HookEvent $fake 'UserPromptSubmit' $a $projA @{ prompt = '로그인 화면을 리팩터링하고 테스트를 추가해 줘'; permission_mode = 'default' }
    $null = Send-HookEvent $fake 'PreToolUse' $a $projA @{ tool_name = 'Bash'; tool_input = @{ command = 'npm test' }; tool_use_id = 'toolu_ci_a' }
    $null = Send-HookEvent $fake 'SessionStart' $b $projB @{ source = 'startup'; model = 'claude-opus-5-5' }
    $null = Send-HookEvent $fake 'UserPromptSubmit' $b $projB @{ prompt = '배포 스크립트를 점검해 줘'; permission_mode = 'default' }
    $null = Send-HookEvent $fake 'PermissionRequest' $b $projB @{ tool_name = 'Bash'; tool_input = @{ command = 'rm -rf build' } }
    Test-Value '세션 A(demo-proj) state' 'working' (Read-Session $a).state
    Test-Value '세션 B(api-server) state' 'waiting' (Read-Session $b).state
  }

  Invoke-Scenario -Name '화면 2) 보드 창' -Body {
    [void][CbWin]::SetCursorPos(1, 1)
    $p = Start-Process -FilePath $app -PassThru
    $CbState.BoardPid = $p.Id
    $win = Wait-Until -TimeoutSec 40 -Condition {
      $hs = [CbWin]::VisibleWindows([uint32]$p.Id)
      if ($hs.Length -gt 0) { [pscustomobject]@{ Hwnd = $hs[0]; All = $hs } }
    }
    Test-Value '보드 창이 40초 안에 보임' $true ($null -ne $win)
    if ($null -eq $win) {
      Test-Value '보드 프로세스가 살아 있음' $true (Test-ProcessAlive $p.Id)
      $null = Save-Screenshot 'screen-01-board.png'
      return
    }
    Test-Value '보드 창' '' (@($win.All | ForEach-Object { Format-Window $_ }) -join ' / ') -Info
    Test-Value '보드 창 제목' 'Claude Board' ([CbWin]::Title($win.Hwnd)) -Info
    Start-Sleep -Seconds 6   # WebView2 가 그리고 세션을 한 번 읽을 시간
    Test-Value '보드가 포그라운드(인자 없이 띄우면 포커스를 가짐)' (Format-Window $win.Hwnd) (Format-Window ([CbWin]::Foreground())) -Info
    $null = Save-Screenshot 'screen-01-board.png'   # 트레이 아이콘은 이 화면의 작업 표시줄에서 본다
  }

  Invoke-Scenario -Name '화면 3) 단일 실행' -Body {
    $p2 = Start-Process -FilePath $app -PassThru
    $null = $p2.Handle
    $exited = $p2.WaitForExit(20000)
    Test-Value '두 번째 실행이 20초 안에 끝남' $true $exited
    if ($exited) { Test-Value '두 번째 실행 종료 코드' 0 $p2.ExitCode -Info } else { Stop-Process -Id $p2.Id -Force -ErrorAction SilentlyContinue }
    Test-Value '첫 보드 프로세스가 남아 있음' $true (Test-ProcessAlive $CbState.BoardPid)
    $boards = @(Get-BoardProcesses | Where-Object { "$($_.CommandLine)" -notmatch '--notify' })
    Test-Value '떠 있는 보드 프로세스 수' 1 $boards.Count
    Start-Sleep -Seconds 1
    $null = Save-Screenshot 'screen-02-second-instance.png'
  }

  Invoke-Scenario -Name '화면 4) 알림 패널 — 승인 대기(Notification)' -Body {
    $CbState.PanelWait = Test-NotifyPanel -Label '승인 대기 패널' -EventName 'Notification' -SessionId $CbState.SidA -Cwd $CbState.ProjA -Extra @{ message = 'Claude needs your permission to use Bash'; notification_type = 'permission_prompt' } -Shot 'screen-03-notify-wait.png'
  }

  Invoke-Scenario -Name '화면 5) 알림 패널 — 작업 완료(Stop)' -Body {
    $CbState.PanelDone = Test-NotifyPanel -Label '작업 완료 패널' -EventName 'Stop' -SessionId $CbState.SidB -Cwd $CbState.ProjB -Extra @{ stop_hook_active = $false; last_assistant_message = '배포 스크립트를 점검했습니다. 고칠 곳은 없습니다.' } -Shot 'screen-04-notify-done.png'
  }

  Invoke-Scenario -Name '화면 6) 패널이 스스로 닫히고 보드는 남음' -Body {
    $pids = @(@($CbState.PanelWait, $CbState.PanelDone) | Where-Object { $_ } | ForEach-Object { $_.Pid })
    if ($pids.Count -gt 0) {
      # 표시 10초 + 사라지는 애니메이션. 프론트가 멈추면 watchdog 이 대신 끝낸다(준비 15초·끝날 시각 + 10초)
      $gone = Wait-Until -TimeoutSec 45 -Condition { @($pids | Where-Object { Test-ProcessAlive $_ }).Count -eq 0 }
      Test-Value '알림 패널 프로세스가 45초 안에 스스로 끝남' $true ([bool]$gone)
    }
    Test-Value '보드 프로세스가 남아 있음' $true (Test-ProcessAlive $CbState.BoardPid)
    Test-Value '세션 A state(승인 대기 뒤)' 'waiting' (Read-Session $CbState.SidA).state -Info
    Test-Value '세션 B state(작업 완료 뒤)' 'idle' (Read-Session $CbState.SidB).state -Info
    $null = Save-Screenshot 'screen-05-board-after.png'
    Show-File (Join-Path $CbNotifier 'board.log') -Tail 40
  }
} finally {
  Stop-AllFakeClaude
  Stop-ClaudeBoardProcesses
  Write-CheckSummary -Title '화면 확인(정보용)' -FileName 'screen-checks.md'
  Stop-Transcript | Out-Null
}
Complete-Checks -Title '화면 확인'
$global:LASTEXITCODE = 0
