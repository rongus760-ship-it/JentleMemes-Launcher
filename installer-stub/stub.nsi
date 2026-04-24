; JentleMemes Launcher — NSIS Stub
; Self-extracting wrapper: extracts to %TEMP%, installs WebView2 if needed,
; launches the Tauri installer UI.

!include "x64.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

Name "JentleMemes Launcher Setup"
OutFile "..\dist\JentleMemes-Launcher-Setup.exe"
Unicode True
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 64
SilentInstall silent
AutoCloseWindow true
ShowInstDetails nevershow

Var TEMP_EXTRACT
Var WV2_INSTALLED

Section "Main"
    GetTempFileName $TEMP_EXTRACT
    Delete "$TEMP_EXTRACT"
    CreateDirectory "$TEMP_EXTRACT"

    ; Extract payload
    SetOutPath "$TEMP_EXTRACT\payload"
    File /r "payload\*.*"

    ; Extract installer + WebView2
    SetOutPath "$TEMP_EXTRACT"
    File "jentlememes-installer.exe"
    File "MicrosoftEdgeWebview2Setup.exe"

    ; --- Check for WebView2 Runtime ---
    StrCpy $WV2_INSTALLED "0"

    SetRegView 64
    ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-235B8DE58920}" "pv"
    ${If} $0 != ""
        StrCpy $WV2_INSTALLED "1"
    ${EndIf}

    ${If} $WV2_INSTALLED == "0"
        SetRegView 32
        ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-235B8DE58920}" "pv"
        ${If} $0 != ""
            StrCpy $WV2_INSTALLED "1"
        ${EndIf}
    ${EndIf}

    ${If} $WV2_INSTALLED == "0"
        SetRegView 64
        ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-235B8DE58920}" "pv"
        ${If} $0 != ""
            StrCpy $WV2_INSTALLED "1"
        ${EndIf}
    ${EndIf}

    ${If} $WV2_INSTALLED == "0"
        ExecWait '"$TEMP_EXTRACT\MicrosoftEdgeWebview2Setup.exe" /silent /install'
    ${EndIf}

    ; --- Launch installer UI ---
    ExecWait '"$TEMP_EXTRACT\jentlememes-installer.exe" --payload "$TEMP_EXTRACT\payload"'

    ; --- Cleanup ---
    RMDir /r "$TEMP_EXTRACT"
SectionEnd
