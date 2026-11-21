; Wrap the Windows tools and image recipe in a single per-user launcher.
Unicode true
RequestExecutionLevel user
SilentInstall silent
SetCompressor /SOLID lzma
Name "Tizen Ornn"
Icon "art\tizen-ornn.ico"
OutFile "${PAYLOAD}\tizen-ornn-windows-x64.exe"

Section
  StrCpy $0 "$LOCALAPPDATA\Tizen Ornn\tools"
  StrCpy $1 "$0\${BUNDLE_ID}"
  IfFileExists "$1\tizen-ornn-gui.exe" launch
  ClearErrors
  CreateDirectory "$0"
  GetTempFileName $2 "$0"
  IfErrors failed
  Delete "$2"
  SetOutPath "$2"
  IfErrors failed
  File "..\LICENSE"
  File /oname=Barlow-OFL.txt "art\fonts\OFL.txt"
  File "${PAYLOAD}\tizen-ornn.exe"
  File "${PAYLOAD}\tizen-ornn-gui.exe"
  SetOutPath "$2\builder"
  File "${PAYLOAD}\builder\bzImage"
  File "${PAYLOAD}\builder\tizen-ornn_amd64.deb"
  SetOutPath "$0"
  Rename "$2" "$1"
  IfErrors 0 launch
  ; A second launch may have installed the same complete bundle.
  RMDir /r "$2"
  IfFileExists "$1\tizen-ornn-gui.exe" launch failed
launch:
  ClearErrors
  Exec '"$1\tizen-ornn-gui.exe"'
  IfErrors failed
  Goto done
failed:
  MessageBox MB_ICONSTOP "Could not open Tizen Ornn. Check available disk space and write access to $0."
  SetErrorLevel 1
done:
SectionEnd
