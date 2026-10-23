; Crochets de l'installateur Windows (bundle.windows.nsis.installerHooks).

; Le dossier de l'éditeur, Program Files\Flux Croisés, est retiré s'il est
; vide après la désinstallation (RMDir sans /r ne supprime rien d'autre).
!macro NSIS_HOOK_POSTUNINSTALL
  RMDir "$PROGRAMFILES64\${MANUFACTURER}"
  RMDir "$PROGRAMFILES\${MANUFACTURER}"
!macroend
