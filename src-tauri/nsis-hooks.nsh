!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /IM deadline-panel.exe /F'
  nsExec::ExecToLog 'taskkill /IM adhd-deadline-panel.exe /F'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'taskkill /IM deadline-panel.exe /F'
  nsExec::ExecToLog 'taskkill /IM adhd-deadline-panel.exe /F'
!macroend
