#!/bin/sh
# 사용: case.sh <이름> <실행 스크립트> — 시험 창을 열고, 시작 화면·답변 화면을 찍고, 끝내고 닫는다
D=$(dirname "$0"); N=$1; RUN=$2
R=$(osascript -e "tell application \"Ghostty\"
  set cfg to new surface configuration
  set initial working directory of cfg to \"$D/work\"
  set command of cfg to \"/bin/zsh -l $RUN\"
  set wait after command of cfg to true
  set w to new window with configuration cfg
  activate window w
  set t to focused terminal of selected tab of w
  return (id of w) & \"|\" & (id of t)
end tell"); W=${R%%|*}; T=${R##*|}
sleep 10; "$D/shot" "$D/$N-start.png" >/dev/null
osascript -e "set p to read POSIX file \"$D/prompt.txt\" as «class utf8»
tell application \"Ghostty\"
  repeat with t in terminals
    if (id of t as text) is \"$T\" then
      input text p to t
      delay 0.5
      send key \"enter\" to t
    end if
  end repeat
end tell" >/dev/null
sleep 30; "$D/shot" "$D/$N-answer.png" >/dev/null
osascript -e "tell application \"Ghostty\"
  repeat with t in terminals
    if (id of t as text) is \"$T\" then
      input text \"/exit\" to t
      delay 0.4
      send key \"enter\" to t
    end if
  end repeat
end tell" >/dev/null
sleep 4
osascript -e "tell application \"Ghostty\"
  repeat with w in windows
    if (id of w) is \"$W\" then
      close window w
      exit repeat
    end if
  end repeat
end tell" >/dev/null
echo "$N done"
