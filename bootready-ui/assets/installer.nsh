!macro customUnInstall
  ; boot-core 프로세스 종료
  nsExec::Exec 'taskkill /IM boot-core.exe /F'
  ; 레지스트리 시작프로그램 제거
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "BootReady"
  ; AppData 폴더 제거
  RMDir /r "$APPDATA\BootReady"
!macroend
