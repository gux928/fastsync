!define APP_NAME "rrsync"
!define BIN_NAME "rrsync.exe"
!define INSTALL_DIR "$PROGRAMFILES64\${APP_NAME}"

Name "${APP_NAME}"
OutFile "dist\rrsync-setup.exe"
InstallDir "${INSTALL_DIR}"
RequestExecutionLevel admin

Page directory
Page instfiles

Section "Install"
    SetOutPath "$INSTDIR"
    
    # 复制二进制文件并重命名
    File "/oname=${BIN_NAME}" "dist\rrsync-windows-amd64.exe"
    
    # 创建卸载程序
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    # 添加到系统的 PATH 环境变量 (通过注册表)
    # 读取当前的 PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    # 检查是否已经在 PATH 中
    Push $INSTDIR
    Push $0
    Call StrStr
    Pop $1
    StrCmp $1 "" 0 done
    
    # 写入新的 PATH
    WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0;$INSTDIR"
    
    # 通知系统环境变量已更改
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

done:
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\${BIN_NAME}"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    
    # 注意：卸载时自动清理 PATH 比较复杂且有风险，通常建议保留或手动清理
    # 这里为了安全不自动修改注册表 PATH
SectionEnd

# 简单的字符串搜索函数
Function StrStr
  Exch $R1 ; 搜索内容
  Exch
  Exch $R2 ; 目标字符串
  Push $R3
  Push $R4
  Push $R5
  StrLen $R3 $R1
  StrLen $R4 $R2
  IntOp $R4 $R4 - $R3
  StrCpy $R5 0
  loop:
    StrCpy $R0 $R2 $R3 $R5
    StrCmp $R0 $R1 done
    IntOp $R5 $R5 + 1
    IntCmp $R5 $R4 loop loop done
  done:
  StrCpy $R1 $R2 "" $R5
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Exch $R1
FunctionEnd
