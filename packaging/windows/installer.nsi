!define APP_NAME "fastsync"
!define BIN_NAME "fastsync.exe"
!define INSTALL_DIR "$PROGRAMFILES64\${APP_NAME}"

Name "${APP_NAME}"
OutFile "..\..\dist\fastsync-setup.exe"

InstallDir "${INSTALL_DIR}"
RequestExecutionLevel admin

Page directory
Page instfiles

Section "Install"
    SetOutPath "$INSTDIR"
    
    # 路径相对于此脚本所在位置
    File "/oname=${BIN_NAME}" "..\..\dist\fastsync-windows-amd64.exe"
    
    # 创建卸载程序
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    # 使用 PowerShell 安全添加环境变量
    # 注意：外层使用双引号，内部 PowerShell 字符串使用单引号
    ExecWait "powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -Command $\"& { $$t = '$INSTDIR'; $$p = [Environment]::GetEnvironmentVariable('Path', 'Machine'); if ($$p -split ';' -notcontains $$t) { [Environment]::SetEnvironmentVariable('Path', $$p + ';' + $$t, 'Machine'); } }$\""

done:
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\${BIN_NAME}"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    
    # 卸载时可选：移除 PATH (同样使用 PowerShell)
    # 考虑到复杂性，这里暂时不自动移除，避免误删
SectionEnd
