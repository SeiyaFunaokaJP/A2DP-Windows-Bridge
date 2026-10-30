; BlueCaster installer (Inno Setup 6.5 or later)
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; Built by installer/build-release.ps1 from the unpacked portable package, so
; both contain the same files:
;   ISCC /DAppVersion=<ver> /DAppVersionNumeric=<x.y.z> /DArch=<x64|arm64>
;        /DPkgDir=<dir> /DWebView2Setup=<MicrosoftEdgeWebview2Setup.exe>
;        /DOutputDir=<dir> /DOutputBaseFilename=<name> BlueCaster.iss
;
; Until 2.0.x the app was "A2DP Windows Bridge" (A2DPWB). 3.0.0 is BlueCaster,
; with an AppId of its own: the earlier versions are uninstalled first, and
; the app takes their settings over at its first start.
;
; Install:
;   - a running core is closed cleanly (`bluecaster-core.exe --quit`: the
;     stream stops and the adapter is released) and started again after;
;     with no installed copy to ask (a new installation beside a portable or
;     development build), Setup's own copy from {tmp} asks it;
;   - A2DP Windows Bridge 2.0.x of this setup (Inno Setup, AppId OldV2Guid) is
;     uninstalled: its core is asked to exit (`a2dpwb-core.exe --quit` in its
;     folder), then its uninstaller runs silently, which keeps the user data
;     and the adapters on WinUSB (it asks nothing in silent mode). Its folder
;     (%LOCALAPPDATA%\A2DP Windows Bridge) is not reused: BlueCaster goes to
;     %LOCALAPPDATA%\BlueCaster. Its settings folder %APPDATA%\A2DPWB is taken
;     over into %APPDATA%\BlueCaster by the app at its first start;
;   - a running v1 (its mutex A2DPWB_SingleInstance) is asked to exit, and v1
;     (Inno Setup, V1UninstKey) is uninstalled. Its uninstaller removes the
;     program files only: the settings folder %APPDATA%\A2DPWB (profiles,
;     pairings, firmware) stays, and the app takes it over as above;
;   - 2.0.0 to 2.0.2-beta.5 (Tauri's NSIS installer, NsisUninstKey) is
;     uninstalled before the files are installed, with its uninstaller in
;     update mode: the adapters stay on WinUSB, the settings stay. An
;     installation that fails or is cancelled after this leaves nothing
;     installed; running the setup again installs it;
;   - "start with Windows" (HKCU Run, value BlueCaster) is pointed at
;     `bluecaster-core.exe --background` when it was on: also when the old
;     value A2DPWB pointed at a 2.0.x core (it is deleted then) or at v1 (v1's
;     uninstaller deletes it). An A2DPWB value pointing anywhere else stays;
;   - the old shortcuts ("A2DP Windows Bridge") are deleted, and made again as
;     BlueCaster when the old ones were there;
;   - the WebView2 Runtime is installed where it is missing;
;   - a new installation asks for the language (AskLanguage, preselected from
;     the language chosen for 2.0.x, else from the Windows display language);
;     an update of this installation uses the one chosen before without asking
;     (UsePreviousLanguage), and so do /SILENT, /VERYSILENT, /P and /LANG=.
;     Setup writes it to HKCU\Software\BlueCaster, value SetupLanguage, where
;     the core takes its language from while the settings have none
;     (settings.rs, default_language); 2.0.x's HKCU\Software\A2DPWB goes;
;   - an update (of this installation, of 2.0.x, of the NSIS one or of v1)
;     says so on the wizard's title and pages (UpdateWording).
; Uninstall: a page of the uninstall wizard asks whether the adapters BlueCaster
; put on WinUSB go back to Windows (one UAC prompt) and whether
; %APPDATA%\BlueCaster (settings, codecs, pairings, firmware) and the window's
; own data (%LOCALAPPDATA%\io.github.seiyafunaokajp.bluecaster) are deleted,
; both on by default (InitializeUninstallProgressForm), with what 2.0.x may
; have left of them (%APPDATA%\A2DPWB, %LOCALAPPDATA%\io.github.seiyafunaokajp.a2dpwb);
; the core is closed; the Run value is removed when it points at this
; installation.
;
; Kept only for the step from 2.0.x (OldV2*, NsisUninstKey, V1*, OldCoreMutex,
; OldAppMutex, OldRunValue, OldLanguageKey, the [InstallDelete] of the old
; shortcuts, the A2DPWB data folders on uninstall): they can go 6 to 12 months
; after 3.0.0, once nobody updates from 2.0.x any more.
;
; Command line, besides Inno Setup's own (/SILENT, /VERYSILENT, /DIR=, ...):
;   /UPDATE  the in-app update (app/src/ui/src-tauri/src/update.rs): starts
;            bluecaster.exe again when done;
;   /P       NSIS's passive mode, which the updater of 2.0.2-beta.5 and
;            earlier passes: no pages, only the progress (like /SILENT);
;   /KEEPV1  leaves an installed v1, a running v1 and the Run value alone
;            (testing v2 beside an installed v1; both use the same settings
;            folder). The core still replaces v1 when it starts: it asks a
;            running v1 to exit and takes over v1's Run value (resident.rs,
;            refresh_autostart).
;
; Saved as UTF-8 with a BOM (the messages below).

#ifndef AppVersion
  #error AppVersion is not defined
#endif
#ifndef AppVersionNumeric
  #define AppVersionNumeric AppVersion
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef PkgDir
  #error PkgDir is not defined
#endif
#ifndef WebView2Setup
  #error WebView2Setup is not defined
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "BlueCaster-" + AppVersion + "-setup"
#endif

#define AppName "BlueCaster"
#define AppExe "bluecaster.exe"
#define CoreExe "bluecaster-core.exe"
#define DriverExe "bluecaster-driver.exe"
; tauri.conf.json's identifier: the windows' notifications and taskbar
; button belong to the shortcut with this AppUserModelID
#define AppUserModelId "io.github.seiyafunaokajp.bluecaster"
; Tauri's identifier: the window's WebView2 data is in %LOCALAPPDATA%\<this>
#define AppIdentifier "io.github.seiyafunaokajp.bluecaster"
; AppId without its braces (BlueCaster 3.0.0 and later; 2.0.x's is OldV2Guid)
#define AppGuid "C7A109ED-F618-4214-8186-E4472C3BDD10"
#define RepoUrl "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge"

[Setup]
; Never change AppId: it ties updates and the uninstaller to earlier installs.
; It changed once, at 3.0.0 (the rename to BlueCaster: a new name, folder and
; Apps entry): A2DP Windows Bridge 2.0.x has {DE4C0AE2-...} (OldV2Guid), which
; this setup uninstalls; v1 has {4D8EA889-0FE6-4753-A25F-2C1A3E4BF658}
; (V1UninstKey).
AppId={{{#AppGuid}}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Seiya Funaoka
AppPublisherURL={#RepoUrl}
AppSupportURL={#RepoUrl}/issues
AppUpdatesURL={#RepoUrl}/releases
AppCopyright=Copyright (c) Seiya Funaoka. Apache-2.0 OR MIT.
VersionInfoVersion={#AppVersionNumeric}
VersionInfoProductTextVersion={#AppVersion}
; For the current user only (no administrator rights): an installation for
; all users would ask for them at every update
PrivilegesRequired=lowest
; An update of this installation keeps its folder (UsePreviousAppDir); the
; folder of 2.0.x is not reused (DefaultDir)
DefaultDirName={code:DefaultDir}
; A Start Menu folder, offered as a task beside the desktop shortcut on one
; page (issue #86: a page of its own for each was one too many); its
; folder name is not asked
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0
; Ends what still runs from the folder after the core's --quit (the
; window, which exits on the core's Bye); the core is started again in [Run]
CloseApplications=yes
RestartApplications=no
LicenseFile={#PkgDir}\licenses\bluecaster\LICENSE
SetupIconFile=..\app\src\ui\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
; Setup's own Select Language dialog would also ask at every update (it
; comes before any script): AskLanguage asks a new installation instead. An
; update takes the previous one's language (UsePreviousLanguage).
ShowLanguageDialog=no
UsePreviousLanguage=yes
Compression=lzma2/max
SolidCompression=yes
; %TEMP%\Setup Log <date> #<n>.txt, for reports of failed installs
SetupLogging=yes
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "ko"; MessagesFile: "compiler:Languages\Korean.isl"
; Not shipped with Inno Setup 6 (installer/README.md)
Name: "zhcn"; MessagesFile: "languages\ChineseSimplified.isl"
Name: "zhtw"; MessagesFile: "languages\ChineseTraditional.isl"

[CustomMessages]
en.OtherRunning=BlueCaster or A2DP Windows Bridge is still running (v1, or a copy that Setup could not close). Exit it (right-click its tray icon > Exit), then click Retry.%n%nCancel stops the installation.
ja.OtherRunning=BlueCaster または A2DP Windows Bridge（v1、またはセットアップが終了できなかったもの）が起動しています。タスクトレイのアイコンを右クリックして「終了」を選んでから、「再試行」を押してください。%n%n「キャンセル」でインストールを中止します。
de.OtherRunning=BlueCaster oder A2DP Windows Bridge läuft noch (v1 oder eine Kopie, die Setup nicht beenden konnte). Beenden Sie es (Rechtsklick auf das Taskleistensymbol > Beenden) und klicken Sie dann auf Wiederholen.%n%nAbbrechen beendet die Installation.
fr.OtherRunning=BlueCaster ou A2DP Windows Bridge est encore en cours d'exécution (v1, ou une copie que l'installation n'a pas pu fermer). Quittez-le (clic droit sur son icône dans la zone de notification > Quitter), puis cliquez sur Réessayer.%n%nAnnuler arrête l'installation.
es.OtherRunning=BlueCaster o A2DP Windows Bridge sigue en ejecución (v1, o una copia que el instalador no pudo cerrar). Ciérrelo (clic derecho en su icono de la bandeja > Salir) y pulse Reintentar.%n%nCancelar detiene la instalación.
ko.OtherRunning=BlueCaster 또는 A2DP Windows Bridge(v1 또는 설치 프로그램이 종료하지 못한 프로그램)가 실행 중입니다. 트레이 아이콘을 마우스 오른쪽 버튼으로 클릭하고 [종료]를 선택한 다음 [다시 시도]를 누르세요.%n%n[취소]를 누르면 설치를 중단합니다.
zhcn.OtherRunning=BlueCaster 或 A2DP Windows Bridge 仍在运行（v1，或安装程序无法关闭的副本）。请右键单击其托盘图标并选择“退出”，然后单击“重试”。%n%n单击“取消”将停止安装。
zhtw.OtherRunning=BlueCaster 或 A2DP Windows Bridge 仍在執行（v1，或安裝程式無法關閉的副本）。請以滑鼠右鍵按一下其系統匣圖示並選擇「結束」，然後按一下「重試」。%n%n按一下「取消」將停止安裝。

en.V1Left=A2DP Windows Bridge v1 could not be uninstalled. You can remove it later in Settings > Apps; your settings, pairings and firmware stay in either case.
ja.V1Left=A2DP Windows Bridge v1 をアンインストールできませんでした。あとで「設定」>「アプリ」から削除できます。設定、ペアリング、ファームウェアはどちらの場合も残ります。
de.V1Left=A2DP Windows Bridge v1 konnte nicht deinstalliert werden. Sie können es später unter Einstellungen > Apps entfernen; Ihre Einstellungen, Kopplungen und Firmware bleiben in jedem Fall erhalten.
fr.V1Left=A2DP Windows Bridge v1 n'a pas pu être désinstallé. Vous pourrez le supprimer plus tard dans Paramètres > Applications ; vos réglages, appairages et firmware sont conservés dans tous les cas.
es.V1Left=No se pudo desinstalar A2DP Windows Bridge v1. Puede quitarlo más tarde en Configuración > Aplicaciones; su configuración, emparejamientos y firmware se conservan en cualquier caso.
ko.V1Left=A2DP Windows Bridge v1을 제거하지 못했습니다. 나중에 [설정] > [앱]에서 제거할 수 있습니다. 설정, 페어링, 펌웨어는 어느 경우에도 유지됩니다.
zhcn.V1Left=无法卸载 A2DP Windows Bridge v1。您可以稍后在“设置”>“应用”中将其删除；无论如何，您的设置、配对和固件都会保留。
zhtw.V1Left=無法解除安裝 A2DP Windows Bridge v1。您可以稍後在「設定」>「應用程式」中將其移除；無論如何，您的設定、配對和韌體都會保留。

en.V2Left=The previous version, A2DP Windows Bridge 2.0.x, could not be uninstalled. You can remove it later in Settings > Apps; clear "%1" there to keep your settings, pairings and firmware.
ja.V2Left=以前のバージョン（A2DP Windows Bridge 2.0.x）をアンインストールできませんでした。あとで「設定」>「アプリ」から削除できます。設定、ペアリング、ファームウェアを残すには、そのとき「%1」のチェックを外してください。
de.V2Left=Die vorherige Version, A2DP Windows Bridge 2.0.x, konnte nicht deinstalliert werden. Sie können sie später unter Einstellungen > Apps entfernen; deaktivieren Sie dort „%1“, damit Ihre Einstellungen, Kopplungen und Firmware erhalten bleiben.
fr.V2Left=La version précédente, A2DP Windows Bridge 2.0.x, n'a pas pu être désinstallée. Vous pourrez la supprimer plus tard dans Paramètres > Applications ; décochez alors « %1 » pour conserver vos réglages, appairages et firmware.
es.V2Left=No se pudo desinstalar la versión anterior, A2DP Windows Bridge 2.0.x. Puede quitarla más tarde en Configuración > Aplicaciones; desmarque allí «%1» para conservar su configuración, emparejamientos y firmware.
ko.V2Left=이전 버전(A2DP Windows Bridge 2.0.x)을 제거하지 못했습니다. 나중에 [설정] > [앱]에서 제거할 수 있습니다. 설정, 페어링, 펌웨어를 유지하려면 그때 [%1]의 선택을 해제하세요.
zhcn.V2Left=无法卸载以前的版本（A2DP Windows Bridge 2.0.x）。您可以稍后在“设置”>“应用”中将其删除；若要保留您的设置、配对和固件，请在那时取消勾选“%1”。
zhtw.V2Left=無法解除安裝先前的版本（A2DP Windows Bridge 2.0.x）。您可以稍後在「設定」>「應用程式」中將其移除；若要保留您的設定、配對和韌體，請在那時取消勾選「%1」。

en.UninstallChoices=Uninstall options
ja.UninstallChoices=アンインストールのオプション
de.UninstallChoices=Deinstallationsoptionen
fr.UninstallChoices=Options de désinstallation
es.UninstallChoices=Opciones de desinstalación
ko.UninstallChoices=제거 옵션
zhcn.UninstallChoices=卸载选项
zhtw.UninstallChoices=解除安裝選項

en.UninstallChoicesLabel=Choose what is removed together with %1.
ja.UninstallChoicesLabel=%1 と一緒に元に戻すもの、削除するものを選んでください。
de.UninstallChoicesLabel=Wählen Sie, was zusammen mit %1 entfernt wird.
fr.UninstallChoicesLabel=Choisissez ce qui est supprimé avec %1.
es.UninstallChoicesLabel=Elija qué se quita junto con %1.
ko.UninstallChoicesLabel=%1와(과) 함께 제거할 항목을 선택하세요.
zhcn.UninstallChoicesLabel=选择随 %1 一起移除的内容。
zhtw.UninstallChoicesLabel=選擇隨 %1 一起移除的內容。

en.UninstallButton=&Uninstall
ja.UninstallButton=アンインストール(&U)
de.UninstallButton=&Deinstallieren
fr.UninstallButton=&Désinstaller
es.UninstallButton=&Desinstalar
ko.UninstallButton=제거(&U)
zhcn.UninstallButton=卸载(&U)
zhtw.UninstallButton=解除安裝(&U)

en.RestoreChoice=Restore the original Bluetooth driver
ja.RestoreChoice=元の Bluetooth ドライバーに戻す
de.RestoreChoice=Den ursprünglichen Bluetooth-Treiber wiederherstellen
fr.RestoreChoice=Rétablir le pilote Bluetooth d'origine
es.RestoreChoice=Restaurar el controlador Bluetooth original
ko.RestoreChoice=원래 블루투스 드라이버 복원
zhcn.RestoreChoice=恢复原来的蓝牙驱动程序
zhtw.RestoreChoice=還原原本的藍牙驅動程式

en.RestoreChoiceText=Removes the WinUSB driver and restores the Bluetooth adapter to Windows (administrator access is required).
ja.RestoreChoiceText=WinUSB ドライバーを外し、Bluetooth アダプターを Windows に戻します。Windows が管理者の許可を求めます。
de.RestoreChoiceText=Entfernt den WinUSB-Treiber und gibt den Bluetooth-Adapter an Windows zurück. Windows fragt nach Administratorrechten.
fr.RestoreChoiceText=Retire le pilote WinUSB et rend l'adaptateur Bluetooth à Windows. Windows demande les droits d'administrateur.
es.RestoreChoiceText=Quita el controlador WinUSB y devuelve el adaptador Bluetooth a Windows. Windows pedirá permisos de administrador.
ko.RestoreChoiceText=WinUSB 드라이버를 제거하고 블루투스 어댑터를 Windows에 돌려줍니다. Windows가 관리자 권한을 요청합니다.
zhcn.RestoreChoiceText=移除 WinUSB 驱动程序，将蓝牙适配器还给 Windows。Windows 会请求管理员权限。
zhtw.RestoreChoiceText=移除 WinUSB 驅動程式，將藍牙介面卡交還給 Windows。Windows 會要求系統管理員權限。

en.DeleteChoice=Delete user data
ja.DeleteChoice=ユーザーデータを削除する
de.DeleteChoice=Benutzerdaten löschen
fr.DeleteChoice=Supprimer les données utilisateur
es.DeleteChoice=Eliminar los datos de usuario
ko.DeleteChoice=사용자 데이터 삭제
zhcn.DeleteChoice=删除用户数据
zhtw.DeleteChoice=刪除使用者資料

en.DeleteChoiceText=Deletes settings, codecs, device pairings and downloaded firmware (%1).
ja.DeleteChoiceText=設定、コーデック、デバイスのペアリング、ダウンロードしたファームウェアを削除します（%1）。
de.DeleteChoiceText=Löscht Einstellungen, Codecs, Gerätekopplungen und heruntergeladene Firmware (%1).
fr.DeleteChoiceText=Supprime les réglages, les codecs, les appairages des appareils et le firmware téléchargé (%1).
es.DeleteChoiceText=Elimina la configuración, los códecs, los emparejamientos de dispositivos y el firmware descargado (%1).
ko.DeleteChoiceText=설정, 코덱, 기기 페어링, 다운로드한 펌웨어를 삭제합니다(%1).
zhcn.DeleteChoiceText=删除设置、编解码器、设备配对和已下载的固件（%1）。
zhtw.DeleteChoiceText=刪除設定、編解碼器、裝置配對和已下載的韌體（%1）。

en.RestoreFailed=The adapter could not be given back to Windows completely: it may be unplugged, or Windows chose WinUSB again. Plug it in, right-click it in Device Manager, choose Uninstall device, then unplug and replug it (see "By hand in Device Manager" in the setup guide).
ja.RestoreFailed=アダプターを Windows に戻しきれませんでした。アダプターが抜かれているか、Windows がまた WinUSB を選んだ可能性があります。アダプターを挿した状態でデバイス マネージャーを開き、アダプターを右クリックして「デバイスのアンインストール」を選んでから、抜いて挿し直してください（セットアップガイドの「デバイス マネージャーで手動で行う」を参照）。
de.RestoreFailed=Der Adapter konnte nicht vollständig an Windows zurückgegeben werden: Er ist vielleicht nicht eingesteckt, oder Windows hat wieder WinUSB gewählt. Stecken Sie ihn ein, klicken Sie im Geräte-Manager mit der rechten Maustaste darauf, wählen Sie Gerät deinstallieren, und stecken Sie ihn dann aus und wieder ein (siehe „Von Hand im Geräte-Manager“ in der Einrichtungsanleitung).
fr.RestoreFailed=L'adaptateur n'a pas pu être entièrement rendu à Windows : il est peut-être débranché, ou Windows a de nouveau choisi WinUSB. Branchez-le, faites un clic droit dessus dans le Gestionnaire de périphériques, choisissez Désinstaller l'appareil, puis débranchez-le et rebranchez-le (voir « À la main dans le Gestionnaire de périphériques » dans le guide d'installation).
es.RestoreFailed=No se pudo devolver el adaptador a Windows por completo: puede que esté desconectado, o que Windows haya vuelto a elegir WinUSB. Conéctelo, haga clic derecho en él en el Administrador de dispositivos, elija Desinstalar el dispositivo y después desconéctelo y vuelva a conectarlo (consulte «A mano en el Administrador de dispositivos» en la guía de configuración).
ko.RestoreFailed=어댑터를 Windows에 완전히 돌려주지 못했습니다. 어댑터가 분리되었거나 Windows가 다시 WinUSB를 선택했을 수 있습니다. 어댑터를 꽂은 상태에서 장치 관리자를 열고 어댑터를 마우스 오른쪽 버튼으로 클릭한 다음 [디바이스 제거]를 선택하고, 어댑터를 뽑았다가 다시 꽂으세요(설정 가이드의 '장치 관리자에서 직접 하기' 참조).
zhcn.RestoreFailed=未能将适配器完全还给 Windows：适配器可能已拔出，或 Windows 再次选择了 WinUSB。请插入适配器，在设备管理器中右键单击它，选择“卸载设备”，然后拔出并重新插入（参见设置指南中的“在设备管理器中手动操作”）。
zhtw.RestoreFailed=無法將介面卡完全交還給 Windows：介面卡可能已拔除，或 Windows 再次選擇了 WinUSB。請插入介面卡，在裝置管理員中以滑鼠右鍵按一下它，選擇「解除安裝裝置」，然後拔除並重新插入（請參閱設定指南中的「在裝置管理員中手動操作」）。

en.InstallingWebView2=Installing the Microsoft Edge WebView2 Runtime...
ja.InstallingWebView2=Microsoft Edge WebView2 ランタイムをインストールしています...
de.InstallingWebView2=Die Microsoft Edge WebView2-Runtime wird installiert...
fr.InstallingWebView2=Installation du runtime Microsoft Edge WebView2...
es.InstallingWebView2=Instalando el runtime de Microsoft Edge WebView2...
ko.InstallingWebView2=Microsoft Edge WebView2 런타임을 설치하는 중...
zhcn.InstallingWebView2=正在安装 Microsoft Edge WebView2 运行时...
zhtw.InstallingWebView2=正在安裝 Microsoft Edge WebView2 執行階段...

; The Start Menu task, beside Setup's own CreateDesktopIcon (worded as each
; language's CreateDesktopIcon)
en.CreateStartMenuIcon=Create a &Start Menu shortcut
ja.CreateStartMenuIcon=スタートメニューにアイコンを作成する(&S)
de.CreateStartMenuIcon=&Startmenü-Symbol erstellen
fr.CreateStartMenuIcon=Créer une icône dans le menu &Démarrer
es.CreateStartMenuIcon=Crear un acceso directo en el menú &Inicio
ko.CreateStartMenuIcon=시작 메뉴에 바로 가기 만들기(&S)
zhcn.CreateStartMenuIcon=创建开始菜单快捷方式(&S)
zhtw.CreateStartMenuIcon=建立開始功能表圖示(&S)

; The app's language code (app/lang, settings.rs LANGUAGES) of each one
en.AppLanguage=en
ja.AppLanguage=ja
de.AppLanguage=de
fr.AppLanguage=fr
es.AppLanguage=es
ko.AppLanguage=ko
zhcn.AppLanguage=zh-CN
zhtw.AppLanguage=zh-TW

; The Ready page, when an earlier version is uninstalled first (ReadyNote)
en.OldVersionsText=Setup first uninstalls the earlier versions found on this PC (%1). Your settings, device pairings and firmware are kept.
ja.OldVersionsText=セットアップは最初に、この PC にある以前のバージョン（%1）をアンインストールします。設定、デバイスのペアリング、ファームウェアはそのまま残ります。
de.OldVersionsText=Setup deinstalliert zuerst die früheren Versionen auf diesem PC (%1). Ihre Einstellungen, Gerätekopplungen und Firmware bleiben erhalten.
fr.OldVersionsText=L'installation désinstalle d'abord les versions précédentes trouvées sur ce PC (%1). Vos réglages, appairages et firmware sont conservés.
es.OldVersionsText=El instalador desinstala primero las versiones anteriores que hay en este equipo (%1). Su configuración, emparejamientos y firmware se conservan.
ko.OldVersionsText=설치 프로그램은 먼저 이 PC에 있는 이전 버전(%1)을 제거합니다. 설정, 기기 페어링, 펌웨어는 그대로 유지됩니다.
zhcn.OldVersionsText=安装程序会先卸载此电脑上的早期版本（%1）。您的设置、设备配对和固件都会保留。
zhtw.OldVersionsText=安裝程式會先解除安裝此電腦上的舊版本（%1）。您的設定、裝置配對和韌體都會保留。

en.OldV1=A2DP Windows Bridge v1
ja.OldV1=A2DP Windows Bridge v1
de.OldV1=A2DP Windows Bridge v1
fr.OldV1=A2DP Windows Bridge v1
es.OldV1=A2DP Windows Bridge v1
ko.OldV1=A2DP Windows Bridge v1
zhcn.OldV1=A2DP Windows Bridge v1
zhtw.OldV1=A2DP Windows Bridge v1

en.OldNsis=A2DP Windows Bridge 2.0.0 to 2.0.2-beta.5
ja.OldNsis=A2DP Windows Bridge 2.0.0 から 2.0.2-beta.5 まで
de.OldNsis=A2DP Windows Bridge 2.0.0 bis 2.0.2-beta.5
fr.OldNsis=A2DP Windows Bridge 2.0.0 à 2.0.2-beta.5
es.OldNsis=A2DP Windows Bridge 2.0.0 a 2.0.2-beta.5
ko.OldNsis=A2DP Windows Bridge 2.0.0~2.0.2-beta.5
zhcn.OldNsis=A2DP Windows Bridge 2.0.0 至 2.0.2-beta.5
zhtw.OldNsis=A2DP Windows Bridge 2.0.0 至 2.0.2-beta.5

en.OldV2=A2DP Windows Bridge 2.0.x
ja.OldV2=A2DP Windows Bridge 2.0.x
de.OldV2=A2DP Windows Bridge 2.0.x
fr.OldV2=A2DP Windows Bridge 2.0.x
es.OldV2=A2DP Windows Bridge 2.0.x
ko.OldV2=A2DP Windows Bridge 2.0.x
zhcn.OldV2=A2DP Windows Bridge 2.0.x
zhtw.OldV2=A2DP Windows Bridge 2.0.x

en.OldSep=,%_
ja.OldSep=、
de.OldSep=,%_
fr.OldSep=,%_
es.OldSep=,%_
ko.OldSep=,%_
zhcn.OldSep=、
zhtw.OldSep=、

; An update's wording (UpdateWording), in place of Setup's installation messages
en.UpdateTitle=Update - %1
ja.UpdateTitle=アップデート - %1
de.UpdateTitle=Update - %1
fr.UpdateTitle=Mise à jour - %1
es.UpdateTitle=Actualización - %1
ko.UpdateTitle=업데이트 - %1
zhcn.UpdateTitle=更新 - %1
zhtw.UpdateTitle=更新 - %1

en.UpdateButton=&Update
ja.UpdateButton=アップデート(&U)
de.UpdateButton=&Aktualisieren
fr.UpdateButton=&Mettre à jour
es.UpdateButton=&Actualizar
ko.UpdateButton=업데이트(&U)
zhcn.UpdateButton=更新(&U)
zhtw.UpdateButton=更新(&U)

en.UpdateReady=Ready to Update
ja.UpdateReady=アップデート準備完了
de.UpdateReady=Bereit zum Aktualisieren
fr.UpdateReady=Prêt à mettre à jour
es.UpdateReady=Listo para actualizar
ko.UpdateReady=업데이트 준비 완료
zhcn.UpdateReady=准备更新
zhtw.UpdateReady=準備更新

en.UpdateReadyLabel1=Setup is now ready to begin updating %1 on your computer.
ja.UpdateReadyLabel1=%1 のアップデートを開始する準備ができました。
de.UpdateReadyLabel1=Das Setup ist jetzt bereit, %1 auf Ihrem Computer zu aktualisieren.
fr.UpdateReadyLabel1=L'assistant est prêt à mettre à jour %1 sur votre ordinateur.
es.UpdateReadyLabel1=El programa de instalación está listo para actualizar %1 en su equipo.
ko.UpdateReadyLabel1=이제 이 컴퓨터의 %1 업데이트를 시작할 준비가 되었습니다.
zhcn.UpdateReadyLabel1=安装程序现在准备开始更新您电脑上的 %1。
zhtw.UpdateReadyLabel1=安裝程式現在準備開始更新您電腦上的 %1。

en.UpdateReadyLabel2=Click Update to continue with the update, or click Back if you want to review or change any settings.
ja.UpdateReadyLabel2=アップデートを続行するには「アップデート」を、設定を確認または変更するには「戻る」をクリックしてください。
de.UpdateReadyLabel2=Klicken Sie auf „Aktualisieren“, um mit der Aktualisierung fortzufahren, oder auf „Zurück“, um Ihre Einstellungen zu überprüfen oder zu ändern.
fr.UpdateReadyLabel2=Cliquez sur Mettre à jour pour procéder à la mise à jour, ou sur Précédent pour revoir ou modifier une option.
es.UpdateReadyLabel2=Haga clic en Actualizar para continuar con la actualización o en Atrás para revisar o cambiar alguna configuración.
ko.UpdateReadyLabel2=업데이트를 계속하려면 [업데이트]를, 설정을 확인하거나 변경하려면 [뒤로]를 클릭하세요.
zhcn.UpdateReadyLabel2=单击“更新”继续更新，或单击“上一步”查看或更改设置。
zhtw.UpdateReadyLabel2=按一下「更新」繼續更新，或按一下「上一步」檢視或變更設定。

en.UpdatePreparing=Preparing to Update
ja.UpdatePreparing=アップデートの準備中
de.UpdatePreparing=Vorbereitung der Aktualisierung
fr.UpdatePreparing=Préparation de la mise à jour
es.UpdatePreparing=Preparándose para actualizar
ko.UpdatePreparing=업데이트 준비 중
zhcn.UpdatePreparing=正在准备更新
zhtw.UpdatePreparing=正在準備更新

en.UpdatePreparingDesc=Setup is preparing to update %1 on your computer.
ja.UpdatePreparingDesc=%1 をアップデートする準備をしています。
de.UpdatePreparingDesc=Das Setup bereitet die Aktualisierung von %1 auf Ihrem Computer vor.
fr.UpdatePreparingDesc=L'assistant prépare la mise à jour de %1 sur votre ordinateur.
es.UpdatePreparingDesc=El programa de instalación se está preparando para actualizar %1 en su equipo.
ko.UpdatePreparingDesc=이 컴퓨터의 %1을(를) 업데이트할 준비를 하고 있습니다.
zhcn.UpdatePreparingDesc=安装程序正在准备更新您电脑上的 %1。
zhtw.UpdatePreparingDesc=安裝程式正在準備更新您電腦上的 %1。

en.UpdateInstalling=Updating
ja.UpdateInstalling=アップデート中
de.UpdateInstalling=Aktualisieren
fr.UpdateInstalling=Mise à jour en cours
es.UpdateInstalling=Actualizando
ko.UpdateInstalling=업데이트 중
zhcn.UpdateInstalling=正在更新
zhtw.UpdateInstalling=正在更新

en.UpdateInstallingLabel=Please wait while Setup updates %1 on your computer.
ja.UpdateInstallingLabel=%1 をアップデートしています。しばらくお待ちください。
de.UpdateInstallingLabel=Bitte warten Sie, während %1 auf Ihrem Computer aktualisiert wird.
fr.UpdateInstallingLabel=Veuillez patienter pendant que l'assistant met à jour %1 sur votre ordinateur.
es.UpdateInstallingLabel=Espere mientras se actualiza %1 en su equipo.
ko.UpdateInstallingLabel=이 컴퓨터의 %1을(를) 업데이트하는 동안 잠시 기다려 주세요.
zhcn.UpdateInstallingLabel=安装程序正在更新您电脑上的 %1，请稍候。
zhtw.UpdateInstallingLabel=安裝程式正在更新您電腦上的 %1，請稍候。

en.UpdateFinishedHeading=%1 has been updated
ja.UpdateFinishedHeading=%1 のアップデートが完了しました
de.UpdateFinishedHeading=%1 wurde aktualisiert
fr.UpdateFinishedHeading=%1 a été mis à jour
es.UpdateFinishedHeading=%1 se ha actualizado
ko.UpdateFinishedHeading=%1 업데이트 완료
zhcn.UpdateFinishedHeading=%1 已更新
zhtw.UpdateFinishedHeading=%1 已更新

en.UpdateFinishedLabel=Setup has finished updating %1 on your computer to version %2.
ja.UpdateFinishedLabel=%1 をバージョン %2 にアップデートしました。
de.UpdateFinishedLabel=Das Setup hat %1 auf Ihrem Computer auf Version %2 aktualisiert.
fr.UpdateFinishedLabel=L'assistant a mis à jour %1 vers la version %2 sur votre ordinateur.
es.UpdateFinishedLabel=El programa de instalación ha actualizado %1 a la versión %2 en su equipo.
ko.UpdateFinishedLabel=이 컴퓨터의 %1을(를) 버전 %2(으)로 업데이트했습니다.
zhcn.UpdateFinishedLabel=安装程序已将您电脑上的 %1 更新到版本 %2。
zhtw.UpdateFinishedLabel=安裝程式已將您電腦上的 %1 更新至版本 %2。

[Tasks]
; Both checked; an update keeps the previous choice (UsePreviousTasks), an
; update from before the Start Menu task gets it checked
Name: "startmenuicon"; Description: "{cm:CreateStartMenuIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Dirs]
; Left by the NSIS installer's uninstaller when it was not empty: the
; uninstaller removes only the folders Setup created, unless told otherwise
Name: "{app}"; Flags: uninsalwaysuninstall
Name: "{app}\licenses"; Flags: uninsalwaysuninstall

[Files]
Source: "{#PkgDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
; Run only where WebView2 is missing (InstallWebView2)
Source: "{#WebView2Setup}"; Flags: dontcopy

[InstallDelete]
; The shortcuts of A2DP Windows Bridge 2.0.x, made again under BlueCaster's
; name (WantDesktopIcon; the Start Menu task): the Start Menu shortcut of
; the NSIS installer and 2.0.2-beta.6, outside a folder; the Start Menu
; folder of 2.0.2-beta.7 and later (DefaultGroupName was the AppName: the
; program and its uninstaller); the desktop shortcut. 2.0.x's uninstaller
; removes its own; these are what the NSIS one keeps and what a failed
; uninstall leaves.
Type: files; Name: "{autoprograms}\A2DP Windows Bridge.lnk"
Type: files; Name: "{autoprograms}\A2DP Windows Bridge\*.lnk"
Type: dirifempty; Name: "{autoprograms}\A2DP Windows Bridge"
Type: files; Name: "{autodesktop}\A2DP Windows Bridge.lnk"
; The Start Menu task turned off in an update: the folder an earlier
; installation made goes
Type: files; Name: "{group}\*.lnk"; Tasks: not startmenuicon
Type: dirifempty; Name: "{group}"; Tasks: not startmenuicon

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"; Tasks: startmenuicon
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"; Tasks: startmenuicon
; Kept where there is one: the NSIS installer of 2.0.x always made one, and
; its uninstaller leaves it (also after a failed installation, when the
; setup is run again); 2.0.x's (A2DP Windows Bridge.lnk) is made again here
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"; Check: WantDesktopIcon

[Registry]
; The language chosen in Setup, for the core while the settings have none
; (settings.rs, default_language); 2.0.x's HKCU\Software\A2DPWB is deleted
; in CurStepChanged
Root: HKCU; Subkey: "Software\BlueCaster"; ValueType: string; ValueName: "SetupLanguage"; ValueData: "{code:SetupLanguage}"; Flags: uninsdeletevalue uninsdeletekeyifempty

[Run]
; The core (or, after an update, the window) is started in CurStepChanged,
; after InstallWebView2: [Run] comes before ssPostInstall
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent; Check: not IsUpdate

[Code]
const
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  RunValue = 'BlueCaster';
  { This installation's registration (Setup's key for AppId) }
  UninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{{#AppGuid}}_is1';
  LanguageKey = 'Software\BlueCaster';
  { A2DP Windows Bridge 2.0.2-beta.6 to 2.0.x (this setup before 3.0.0, with
    its own AppId): uninstalled first. Its core, its Run value, the window's
    mutex (its identifier io.github.seiyafunaokajp.a2dpwb), its language. }
  OldV2Guid = 'DE4C0AE2-6745-4C09-9478-7FA6C48527F4';
  OldV2UninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{DE4C0AE2-6745-4C09-9478-7FA6C48527F4}_is1';
  OldCoreExe = 'a2dpwb-core.exe';
  OldRunValue = 'A2DPWB';
  OldCoreMutex = 'Local\A2DPWB_v2_Core';
  OldAppMutex = 'io.github.seiyafunaokajp.a2dpwb-sim';
  OldLanguageKey = 'Software\A2DPWB';
  V1UninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{4D8EA889-0FE6-4753-A25F-2C1A3E4BF658}_is1';
  V1Mutex = 'A2DPWB_SingleInstance';
  { Tauri's NSIS installer of 2.0.0 to 2.0.2-beta.5 (installer.nsi:
    UNINSTKEY, MANUPRODUCTKEY, whose default value is the folder) }
  NsisUninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\A2DP Windows Bridge';
  NsisManuKey = 'Software\Seiya Funaoka';
  NsisProductKey = 'Software\Seiya Funaoka\A2DP Windows Bridge';
  { The core's (ipc::core_mutex_name, without a BLUECASTER_INSTANCE suffix)
    and the window's (tauri-plugin-single-instance: <identifier>-sim), and
    those of 2.0.x }
  CoreMutex = 'Local\BlueCaster_Core';
  AppMutexes = 'Local\BlueCaster_Core,Local\A2DPWB_v2_Core,{#AppUserModelId}-sim,io.github.seiyafunaokajp.a2dpwb-sim';
  WebView2Key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

var
  UpdateMode, PassiveMode, KeepV1, CoreWasRunning, RunWasSet, HadDesktopIcon, DeleteData, RestoreChosen: Boolean;
  { Setup replaces an installation: this one, 2.0.x (Inno Setup or NSIS) or v1 }
  IsUpgrade: Boolean;
  { An earlier version is uninstalled first: v1, the NSIS one of 2.0.x,
    2.0.x of this setup (OldV2Guid) }
  OldV1, OldNsis, OldV2: Boolean;
  { The Run value A2DPWB points at a 2.0.x core (or this one): it becomes
    BlueCaster }
  TakeOverOldRun: Boolean;
  { The language chosen before (an app language code, AppLanguage), read
    before 2.0.x's uninstaller deletes it; whether this Setup was given one
    (AskLanguage, /LANG=) }
  PrevLanguage: String;
  LanguageChosen: Boolean;
  { The Ready page's list below its note (ReadyNote) }
  ReadyMemoGap, ReadyMemoBottom: Integer;

function HasParam(const Name: String): Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(ParamStr(I), Name) = 0 then begin
      Result := True;
      Exit;
    end;
end;

function HasParamPrefix(const Prefix: String): Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(Copy(ParamStr(I), 1, Length(Prefix)), Prefix) = 0 then begin
      Result := True;
      Exit;
    end;
end;

{ Where AskLanguage puts the copy of Setup it starts }
function RelaunchDir: String;
begin
  Result := AddBackslash(GetTempDir) + 'BlueCaster-setup';
end;

{ The Select Language dialog of a new installation, preselected with the
  language Setup chose from the Windows display language. Another choice
  starts Setup again with /LANG= (the language cannot change once Setup
  runs): False then, and this Setup ends. Setup's loader keeps its .exe
  locked until this Setup has ended (CreateProcess fails with error 5), so
  a copy in RelaunchDir is started; the next Setup deletes it. }
function AskLanguage: Boolean;
var
  Names, Codes, Captions: TArrayOfString;
  Form: TSetupForm;
  Lbl: TNewStaticText;
  Combo: TNewComboBox;
  OKButton, CancelButton: TNewButton;
  Params, CopyExe: String;
  Bytes: AnsiString;
  I, W, Code: Integer;
begin
  { The [Languages] above, each in its own language }
  Names := ['en', 'ja', 'de', 'fr', 'es', 'ko', 'zhcn', 'zhtw'];
  { Their AppLanguage }
  Codes := ['en', 'ja', 'de', 'fr', 'es', 'ko', 'zh-CN', 'zh-TW'];
  Captions := ['English', '日本語', 'Deutsch', 'Français', 'Español', '한국어', '简体中文', '繁體中文'];
  Form := CreateCustomForm(ScaleX(320), ScaleY(80), False, False);
  try
    Form.Caption := SetupMessage(msgSelectLanguageTitle);
    Lbl := TNewStaticText.Create(Form);
    Lbl.Parent := Form;
    Lbl.Left := ScaleX(10);
    Lbl.Top := ScaleY(10);
    Lbl.Caption := SetupMessage(msgSelectLanguageLabel);
    { Just wide enough for "Select the language to use during the
      installation." on one line (issue #86: 220 cut it, a fixed 320 left
      a wide gap after it); a longer translation wraps at 320, and the form
      grows with it }
    W := Lbl.Width + ScaleX(20);
    if W < ScaleX(220) then
      W := ScaleX(220);
    if W > ScaleX(320) then
      W := ScaleX(320);
    Form.ClientWidth := W;
    Lbl.Width := Form.ClientWidth - ScaleX(20);
    Lbl.WordWrap := True;
    Lbl.AdjustHeight;
    Combo := TNewComboBox.Create(Form);
    Combo.Parent := Form;
    Combo.Style := csDropDownList;
    Combo.Left := ScaleX(10);
    Combo.Top := Lbl.Top + Lbl.Height + ScaleY(8);
    Combo.Width := Form.ClientWidth - ScaleX(20);
    Form.ClientHeight := Combo.Top + Combo.Height + ScaleY(12 + 23 + 10);
    for I := 0 to GetArrayLength(Names) - 1 do begin
      Combo.Items.Add(Captions[I]);
      if Names[I] = ActiveLanguage then
        Combo.ItemIndex := I;
    end;
    { The one chosen for 2.0.x (its setup's AppId is not this one's, so
      Setup did not take it itself) }
    for I := 0 to GetArrayLength(Codes) - 1 do
      if CompareText(Codes[I], PrevLanguage) = 0 then
        Combo.ItemIndex := I;
    OKButton := TNewButton.Create(Form);
    OKButton.Parent := Form;
    OKButton.Caption := SetupMessage(msgButtonOK);
    OKButton.ModalResult := mrOk;
    OKButton.Default := True;
    CancelButton := TNewButton.Create(Form);
    CancelButton.Parent := Form;
    CancelButton.Caption := SetupMessage(msgButtonCancel);
    CancelButton.ModalResult := mrCancel;
    CancelButton.Cancel := True;
    W := Form.CalculateButtonWidth([OKButton.Caption, CancelButton.Caption]);
    OKButton.Width := W;
    CancelButton.Width := W;
    OKButton.Height := ScaleY(23);
    CancelButton.Height := ScaleY(23);
    CancelButton.Left := Form.ClientWidth - ScaleX(10) - W;
    OKButton.Left := CancelButton.Left - ScaleX(6) - W;
    OKButton.Top := Form.ClientHeight - ScaleY(23 + 10);
    CancelButton.Top := OKButton.Top;
    Form.ActiveControl := Combo;
    if Form.ShowModal <> mrOk then begin
      Result := False;
      Exit;
    end;
    Result := (Combo.ItemIndex < 0) or (Names[Combo.ItemIndex] = ActiveLanguage);
    if Result then begin
      LanguageChosen := True;
      Exit;
    end;
    { Setup's own arguments again, but /SL5= (the loader's) }
    Params := '/LANG=' + Names[Combo.ItemIndex];
    for I := 1 to ParamCount do
      if CompareText(Copy(ParamStr(I), 1, 5), '/SL5=') <> 0 then
        Params := Params + ' ' + AddQuotes(ParamStr(I));
    Log('Language ' + Names[Combo.ItemIndex] + ' chosen: starting Setup again with ' + Params);
    { FileCopy fails on it too; its bytes can be read }
    CopyExe := RelaunchDir + '\' + ExtractFileName(ExpandConstant('{srcexe}'));
    if not ForceDirectories(RelaunchDir) or not LoadStringFromFile(ExpandConstant('{srcexe}'), Bytes) or
       not SaveStringToFile(CopyExe, Bytes, False) then begin
      Log('Setup could not be copied to ' + CopyExe + ': staying in ' + ActiveLanguage);
      Result := True;
    end else if not Exec(CopyExe, Params, '', SW_SHOWNORMAL, ewNoWait, Code) then begin
      Log(Format('Setup did not start again (%s): staying in %s', [SysErrorMessage(Code), ActiveLanguage]));
      Result := True;
    end;
  finally
    Form.Free;
  end;
end;

function IsUpdate: Boolean;
begin
  Result := UpdateMode;
end;

function WantDesktopIcon: Boolean;
begin
  Result := WizardIsTaskSelected('desktopicon') or HadDesktopIcon;
end;

{ A new installation's folder. Not the folder of 2.0.x (NSIS or this setup
  before 3.0.0): that one is uninstalled, under its old name. An update of
  this installation keeps its own (UsePreviousAppDir). }
function DefaultDir(Param: String): String;
begin
  Result := ExpandConstant('{localappdata}\{#AppName}');
end;

{ [Registry]: the language chosen in this Setup, else the one chosen before
  (a silent update from 2.0.x runs in the Windows display language) }
function SetupLanguage(Param: String): String;
begin
  if LanguageChosen or (PrevLanguage = '') then
    Result := CustomMessage('AppLanguage')
  else
    Result := PrevLanguage;
end;

{ The settings folder, as the core finds it }
function ConfigDir: String;
begin
  Result := GetEnv('BLUECASTER_CONFIG_DIR');
  if Result = '' then
    Result := ExpandConstant('{userappdata}\BlueCaster');
end;

{ '"C:\...\unins000.exe" /x' -> 'C:\...\unins000.exe' }
function UnquotedPath(S: String): String;
var
  P: Integer;
begin
  S := Trim(S);
  if (Length(S) > 0) and (S[1] = '"') then begin
    Delete(S, 1, 1);
    P := Pos('"', S);
    if P > 0 then
      S := Copy(S, 1, P - 1);
  end;
  Result := S;
end;

{ Waits up to Seconds for the key to go (uninstallers that finish in a copy
  of themselves). True when it went. }
function WaitForKeyGone(const RootKey: Integer; const SubKey: String; const Seconds: Integer): Boolean;
var
  I: Integer;
begin
  for I := 1 to Seconds * 2 do begin
    if not RegKeyExists(RootKey, SubKey) then
      Break;
    Sleep(500);
  end;
  Result := not RegKeyExists(RootKey, SubKey);
end;

{ Asks the running v2 core of this user to exit (the copy in Dir speaks to
  whichever core holds the user's pipe): the stream stops and the adapter is
  released. True when one ran. }
function CloseCoreExe(const Exe: String): Boolean;
var
  Code: Integer;
begin
  Result := False;
  if not FileExists(Exe) then
    Exit;
  { Exit code 0: it ran and has exited; 2: none ran; 1: it still runs }
  if Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code) then begin
    Log(Format('%s --quit: exit code %d', [Exe, Code]));
    Result := (Code = 0) or (Code = 1);
  end;
end;

function CloseCore(const Dir: String): Boolean;
begin
  Result := CloseCoreExe(Dir + '\{#CoreExe}');
end;

{ The core of 2.0.x in Dir (a2dpwb-core.exe): it speaks to 2.0.x's pipe }
function CloseOldCore(const Dir: String): Boolean;
begin
  Result := (Dir <> '') and CloseCoreExe(AddBackslash(Dir) + OldCoreExe);
end;

{ The window ends on the core's Bye; 10 s at most }
procedure WaitForAppExit;
var
  I: Integer;
begin
  for I := 1 to 50 do begin
    if not CheckForMutexes(AppMutexes) then
      Exit;
    Sleep(200);
  end;
  Log('BlueCaster still runs');
end;

{ '' when v1's mutex is free (any more). Otherwise the message the
  installation stops with; in silent mode (nobody to ask) V1Running is set
  instead. A v2 core holds that mutex too (so that v1 cannot start beside
  it): one Setup could not close (an BLUECASTER_INSTANCE test copy, elevated)
  keeps it held, hence the message names both. }
function WaitForV1Exit(var V1Running: Boolean): String;
begin
  Result := '';
  V1Running := False;
  while CheckForMutexes(V1Mutex) do begin
    if WizardSilent then begin
      V1Running := True;
      Exit;
    end;
    if MsgBox(CustomMessage('OtherRunning'), mbError, MB_RETRYCANCEL) = IDCANCEL then begin
      Result := CustomMessage('OtherRunning');
      Exit;
    end;
  end;
end;

{ Uninstalls v1 registered under RootKey (HKCU: installed for the user, HKLM:
  for all users, whose uninstaller asks for administrator rights) }
procedure UninstallV1(const RootKey: Integer);
var
  Uninstaller: String;
  Code: Integer;
begin
  if not RegQueryStringValue(RootKey, V1UninstKey, 'UninstallString', Uninstaller) then
    Exit;
  Uninstaller := UnquotedPath(Uninstaller);
  Log('Uninstalling A2DP Windows Bridge v1: ' + Uninstaller);
  if FileExists(Uninstaller) then
    ShellExec('', Uninstaller, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_SHOW, ewWaitUntilTerminated, Code);
  { Inno Setup's uninstaller finishes in a copy of itself }
  if not WaitForKeyGone(RootKey, V1UninstKey, 30) then begin
    Log(CustomMessage('V1Left'));
    SuppressibleMsgBox(CustomMessage('V1Left'), mbInformation, MB_OK, IDOK);
  end;
end;

{ Uninstalls an installation of Tauri's NSIS installer before this one
  installs, in this folder or another. Its uninstaller in update mode
  (/UPDATE) removes its files and its registration and keeps the shortcuts,
  the Run value, the settings and the adapters' WinUSB driver (installer.nsi
  and hooks.nsh of 2.0.x). An installation that fails or is cancelled after
  this leaves nothing installed: the setup is run again. }
procedure UninstallNsis;
var
  Uninstaller: String;
  Code: Integer;
begin
  if not RegQueryStringValue(HKCU, NsisUninstKey, 'UninstallString', Uninstaller) then
    Exit;
  Uninstaller := UnquotedPath(Uninstaller);
  if FileExists(Uninstaller) then begin
    Log('Uninstalling the NSIS installation: ' + Uninstaller);
    Exec(Uninstaller, '/S /UPDATE', '', SW_HIDE, ewWaitUntilTerminated, Code);
    { NSIS's uninstaller finishes in a copy of itself; it deletes its key
      after the files }
    if not WaitForKeyGone(HKCU, NsisUninstKey, 30) then begin
      Log('The NSIS uninstaller did not finish');
      DeleteFile(Uninstaller);
    end;
  end;
  { Nothing left to uninstall }
  RegDeleteKeyIncludingSubkeys(HKCU, NsisUninstKey);
  RegDeleteKeyIncludingSubkeys(HKCU, NsisProductKey);
  RegDeleteKeyIfEmpty(HKCU, NsisManuKey);
end;

{ The folder of 2.0.x of this setup (OldV2Guid); '' without one }
function OldV2Dir: String;
begin
  if not RegQueryStringValue(HKCU, OldV2UninstKey, 'Inno Setup: App Path', Result) then
    Result := '';
  Result := RemoveBackslashUnlessRoot(Result);
end;

{ Uninstalls A2DP Windows Bridge 2.0.x of this setup before this one
  installs: its core has exited already (PrepareToInstall). Its uninstaller
  in silent mode asks nothing and does nothing besides the uninstall: the
  adapters stay on WinUSB and the user data stays (its
  InitializeUninstallProgressForm turns both off when UninstallSilent), for
  the app to take over. It deletes its shortcuts, its SetupLanguage value
  and its Run value when that points at its core (read before, in
  PrepareToInstall). }
procedure UninstallOldV2;
var
  Uninstaller: String;
  Code: Integer;
begin
  if not RegQueryStringValue(HKCU, OldV2UninstKey, 'UninstallString', Uninstaller) then
    Exit;
  Uninstaller := UnquotedPath(Uninstaller);
  if not FileExists(Uninstaller) then begin
    { Its folder has gone: nothing left to uninstall }
    Log('A2DP Windows Bridge 2.0.x has no uninstaller any more (' + Uninstaller + '): its registration goes');
    RegDeleteKeyIncludingSubkeys(HKCU, OldV2UninstKey);
    Exit;
  end;
  Log('Uninstalling A2DP Windows Bridge 2.0.x: ' + Uninstaller);
  { Nothing that deletes the user data: no page, no choice in silent mode }
  if not Exec(Uninstaller, '/SILENT /NORESTART /SUPPRESSMSGBOXES', '', SW_SHOW, ewWaitUntilTerminated, Code) then
    Log(Format('Its uninstaller did not start: %s', [SysErrorMessage(Code)]));
  { Inno Setup's uninstaller finishes in a copy of itself }
  if WaitForKeyGone(HKCU, OldV2UninstKey, 30) then
    Exit;
  if FileExists(Uninstaller) then begin
    { Kept: Settings > Apps can still remove it }
    Log(CustomMessage('V2Left'));
    SuppressibleMsgBox(FmtMessage(CustomMessage('V2Left'), [CustomMessage('DeleteChoice')]), mbInformation, MB_OK, IDOK);
  end else begin
    Log('The uninstaller of 2.0.x has gone, its registration not: it goes');
    RegDeleteKeyIncludingSubkeys(HKCU, OldV2UninstKey);
  end;
end;

{ Cmd (a Run value) starts one of Exes in Dir }
function RunPointsInto(const Cmd, Dir: String; const Exes: TArrayOfString): Boolean;
var
  I: Integer;
begin
  Result := False;
  if Dir = '' then
    Exit;
  for I := 0 to GetArrayLength(Exes) - 1 do
    if Pos(Lowercase(AddBackslash(Dir) + Exes[I]), Lowercase(Cmd)) > 0 then begin
      Result := True;
      Exit;
    end;
end;

{ The Run value A2DPWB starts a 2.0.x core of an installation that is
  replaced (2.0.x of this setup, the NSIS one) or a core in this folder }
function OldRunIsOurs(const Cmd: String): Boolean;
var
  Exes: TArrayOfString;
  NsisDir: String;
begin
  Exes := [OldCoreExe, 'a2dpwb.exe', '{#CoreExe}', '{#AppExe}'];
  if not RegQueryStringValue(HKCU, NsisProductKey, '', NsisDir) then
    NsisDir := '';
  Result := RunPointsInto(Cmd, OldV2Dir, Exes) or RunPointsInto(Cmd, RemoveBackslashUnlessRoot(NsisDir), Exes) or
    RunPointsInto(Cmd, ExpandConstant('{app}'), Exes);
end;

function WebView2Present: Boolean;
var
  Version: String;
begin
  Result := (RegQueryStringValue(HKLM32, WebView2Key, 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')) or
            (RegQueryStringValue(HKCU, WebView2Key, 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0'));
end;

{ The Evergreen bootstrapper: it downloads the runtime and asks for
  administrator rights itself }
procedure InstallWebView2;
var
  Code: Integer;
begin
  if WebView2Present then
    Exit;
  WizardForm.StatusLabel.Caption := CustomMessage('InstallingWebView2');
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  if ShellExec('', ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'), '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Log(Format('WebView2 bootstrapper: exit code %d', [Code]))
  else
    Log(Format('WebView2 bootstrapper did not start: %s', [SysErrorMessage(Code)]));
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := PassiveMode;
end;

{ An update's pages say "update" where Setup's own messages say "install" }
procedure UpdateWording(CurPageID: Integer);
begin
  case CurPageID of
    wpReady:
      begin
        WizardForm.PageNameLabel.Caption := CustomMessage('UpdateReady');
        WizardForm.PageDescriptionLabel.Caption := FmtMessage(CustomMessage('UpdateReadyLabel1'), ['{#AppName}']);
        WizardForm.ReadyLabel.Caption := CustomMessage('UpdateReadyLabel2');
        WizardForm.NextButton.Caption := CustomMessage('UpdateButton');
      end;
    wpPreparing:
      begin
        WizardForm.PageNameLabel.Caption := CustomMessage('UpdatePreparing');
        WizardForm.PageDescriptionLabel.Caption := FmtMessage(CustomMessage('UpdatePreparingDesc'), ['{#AppName}']);
      end;
    wpInstalling:
      begin
        WizardForm.PageNameLabel.Caption := CustomMessage('UpdateInstalling');
        WizardForm.PageDescriptionLabel.Caption := FmtMessage(CustomMessage('UpdateInstallingLabel'), ['{#AppName}']);
      end;
    wpFinished:
      { Not when it asks for a restart: Setup's text then }
      if not WizardForm.YesRadio.Visible then begin
        WizardForm.FinishedHeadingLabel.Caption := FmtMessage(CustomMessage('UpdateFinishedHeading'), ['{#AppName}']);
        WizardForm.FinishedLabel.Caption :=
          FmtMessage(CustomMessage('UpdateFinishedLabel'), ['{#AppName}', '{#AppVersion}']) + #13#10#13#10 +
          SetupMessage(msgClickFinish);
      end;
  end;
end;

procedure InitializeWizard;
var
  NoIcons, Deselected: String;
begin
  { The Start Menu task alone decides about the folder (there is no Select
    Start Menu Folder page any more): an earlier installation's "Don't create
    a Start Menu folder" turns the task off instead }
  if RegQueryStringValue(HKCU, UninstKey, 'Inno Setup: No Icons', NoIcons) and (NoIcons = '1') then
    WizardSelectTasks('!startmenuicon');
  { 2.0.x of this setup: its choices, as UsePreviousTasks does for this
    AppId (the desktop shortcut: WantDesktopIcon) }
  if OldV2 and not RegKeyExists(HKCU, UninstKey) then begin
    if (RegQueryStringValue(HKCU, OldV2UninstKey, 'Inno Setup: No Icons', NoIcons) and (NoIcons = '1')) or
       (RegQueryStringValue(HKCU, OldV2UninstKey, 'Inno Setup: Deselected Tasks', Deselected) and
        (Pos(',startmenuicon,', ',' + Lowercase(Deselected) + ',') > 0)) then
      WizardSelectTasks('!startmenuicon');
    if RegQueryStringValue(HKCU, OldV2UninstKey, 'Inno Setup: Deselected Tasks', Deselected) and
       (Pos(',desktopicon,', ',' + Lowercase(Deselected) + ',') > 0) then
      WizardSelectTasks('!desktopicon');
  end;
  WizardForm.NoIconsCheck.Checked := False;
  if not IsUpgrade then
    Exit;
  { As SetupWindowTitle, with AppVerName }
  WizardForm.Caption := FmtMessage(CustomMessage('UpdateTitle'), ['{#AppName} {#AppVersion}']);
  { The NSIS installer always made a desktop shortcut: one the user deleted
    stays deleted (this installation's update keeps the previous choice
    itself, UsePreviousTasks) }
  if RegKeyExists(HKCU, NsisUninstKey) and not RegKeyExists(HKCU, UninstKey) and
     not FileExists(ExpandConstant('{autodesktop}\A2DP Windows Bridge.lnk')) then
    WizardSelectTasks('!desktopicon');
end;

{ The Ready page says that the earlier versions found go first, under its
  label, in plain wrapped text (issue #86: say plainly that they go; the
  list below cut off the end of a longer text). The list keeps its bottom.
  Runs on every visit of the page: UpdateWording sets the label anew. }
procedure ReadyNote;
var
  Found, Sep: String;
begin
  if not (OldV1 or OldNsis or OldV2) then
    Exit;
  if ReadyMemoBottom = 0 then begin
    ReadyMemoGap := WizardForm.ReadyMemo.Top - (WizardForm.ReadyLabel.Top + WizardForm.ReadyLabel.Height);
    ReadyMemoBottom := WizardForm.ReadyMemo.Top + WizardForm.ReadyMemo.Height;
  end;
  Sep := CustomMessage('OldSep');
  StringChangeEx(Sep, '%_', ' ', True);
  Found := '';
  if OldV1 then
    Found := CustomMessage('OldV1');
  if OldNsis then begin
    if Found <> '' then
      Found := Found + Sep;
    Found := Found + CustomMessage('OldNsis');
  end;
  if OldV2 then begin
    if Found <> '' then
      Found := Found + Sep;
    Found := Found + CustomMessage('OldV2');
  end;
  WizardForm.ReadyLabel.Caption := WizardForm.ReadyLabel.Caption + #13#10#13#10 +
    FmtMessage(CustomMessage('OldVersionsText'), [Found]);
  WizardForm.ReadyLabel.AdjustHeight;
  WizardForm.ReadyMemo.Top := WizardForm.ReadyLabel.Top + WizardForm.ReadyLabel.Height + ReadyMemoGap;
  WizardForm.ReadyMemo.Height := ReadyMemoBottom - WizardForm.ReadyMemo.Top;
end;

{ ...except the pages Setup shows anyway: Ready, the list of programs to
  close (closed, the default) and Finished. Next is clicked once the page
  is up (BM_CLICK posted: a click during the page change is lost). }
procedure CurPageChanged(CurPageID: Integer);
begin
  if IsUpgrade then
    UpdateWording(CurPageID);
  if CurPageID = wpReady then
    ReadyNote;
  if PassiveMode and ((CurPageID = wpReady) or (CurPageID = wpPreparing) or (CurPageID = wpFinished)) then
    PostMessage(WizardForm.NextButton.Handle, $00F5, 0, 0);
end;

function InitializeSetup: Boolean;
begin
  UpdateMode := HasParam('/UPDATE');
  KeepV1 := HasParam('/KEEPV1');
  Result := True;
  { An earlier AskLanguage's copy (this Setup's own, if it is one, is
    locked and stays) }
  DelTree(RelaunchDir, True, True, True);
  { The updater of 2.0.2-beta.5 and earlier runs the new installer as an
    NSIS one: "/P /UPDATE /R /ARGS". Silent mode cannot be turned on here,
    and Setup's loader keeps its .exe locked, so it cannot start itself
    again with /SILENT either: every page is skipped instead (ShouldSkipPage),
    which leaves the progress, as NSIS's passive mode did. }
  PassiveMode := HasParam('/P') and not WizardSilent;
  if PassiveMode then
    Log('NSIS passive mode (/P): the pages are skipped');
  OldV2 := RegKeyExists(HKCU, OldV2UninstKey);
  IsUpgrade := RegKeyExists(HKCU, UninstKey) or RegKeyExists(HKCU, NsisUninstKey) or OldV2 or
    (not KeepV1 and (RegKeyExists(HKCU, V1UninstKey) or RegKeyExists(HKLM, V1UninstKey)));
  if IsUpgrade then
    Log('Updating an earlier installation');
  OldNsis := RegKeyExists(HKCU, NsisUninstKey);
  { Before 2.0.x's uninstaller deletes it }
  if not RegQueryStringValue(HKCU, LanguageKey, 'SetupLanguage', PrevLanguage) or (Trim(PrevLanguage) = '') then
    if not RegQueryStringValue(HKCU, OldLanguageKey, 'SetupLanguage', PrevLanguage) then
      PrevLanguage := '';
  PrevLanguage := Trim(PrevLanguage);
  LanguageChosen := HasParamPrefix('/LANG=');
  OldV1 := not KeepV1 and (RegKeyExists(HKCU, V1UninstKey) or RegKeyExists(HKLM, V1UninstKey));
  { Asked once: an update of this installation has its language. Nobody to
    ask in silent and passive mode. }
  if not RegKeyExists(HKCU, UninstKey) and not WizardSilent and not PassiveMode and
     not HasParamPrefix('/LANG=') then
    Result := AskLanguage;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  NsisDir, OldRun: String;
  V1Running: Boolean;
begin
  Result := '';
  CoreWasRunning := CloseCore(ExpandConstant('{app}'));
  HadDesktopIcon := FileExists(ExpandConstant('{autodesktop}\{#AppName}.lnk')) or
    FileExists(ExpandConstant('{autodesktop}\A2DP Windows Bridge.lnk'));
  if RegKeyExists(HKCU, NsisUninstKey) then begin
    if RegQueryStringValue(HKCU, NsisProductKey, '', NsisDir) and (NsisDir <> '') and
       CloseOldCore(NsisDir) then
      CoreWasRunning := True;
  end;
  { 2.0.x of this setup: its core (a2dpwb-core.exe, 2.0.x's pipe) }
  if OldV2 and CloseOldCore(OldV2Dir) then
    CoreWasRunning := True;
  { A core still runs with no installed copy to ask (a new installation
    beside a portable or development build): this Setup's own copy asks it.
    It holds v1's mutex as well, which read as "v1 is running" below. A
    2.0.x core that runs from elsewhere is not asked (another pipe): it
    reads as "v1 is running" (OtherRunning). }
  if CheckForMutexes(CoreMutex) then begin
    try
      ExtractTemporaryFile('{#CoreExe}');
      if CloseCore(ExpandConstant('{tmp}')) then
        CoreWasRunning := True;
    except
      Log('Extracting the core to close the running one failed: ' + GetExceptionMessage);
    end;
  end;
  if CheckForMutexes(OldCoreMutex) then
    Log('A core of A2DP Windows Bridge 2.0.x still runs');

  { "Start with Windows": BlueCaster's value, or 2.0.x's (A2DPWB) when it
    starts a core of an installation replaced here, read before 2.0.x's
    uninstaller deletes it. An A2DPWB value that starts anything else is
    left alone, but v1's (below). }
  RunWasSet := RegValueExists(HKCU, RunKey, RunValue);
  TakeOverOldRun := RegQueryStringValue(HKCU, RunKey, OldRunValue, OldRun) and OldRunIsOurs(OldRun);
  if TakeOverOldRun then begin
    Log('The Run value ' + OldRunValue + ' (' + OldRun + ') becomes ' + RunValue);
    RunWasSet := True;
  end;
  if not KeepV1 then begin
    { v1's (A2DPWB.exe, value A2DPWB), read before v1's uninstaller deletes it }
    if not TakeOverOldRun and RegQueryStringValue(HKCU, RunKey, OldRunValue, OldRun) and
       (Pos('\a2dpwb.exe', Lowercase(OldRun)) > 0) then
      RunWasSet := True;
    Result := WaitForV1Exit(V1Running);
    if Result <> '' then
      Exit;
    if V1Running then
      Log('A2DP Windows Bridge v1 is running: it stays installed')
    else begin
      UninstallV1(HKCU);
      UninstallV1(HKLM);
    end;
  end;

  { Last, once nothing above can stop the installation }
  if RegKeyExists(HKCU, NsisUninstKey) then begin
    WaitForAppExit;
    UninstallNsis;
  end;
  if RegKeyExists(HKCU, OldV2UninstKey) then begin
    WaitForAppExit;
    UninstallOldV2;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
  OldRun: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  if RunWasSet then
    RegWriteStringValue(HKCU, RunKey, RunValue, '"' + ExpandConstant('{app}\{#CoreExe}') + '" --background');
  { 2.0.x's value, unless its uninstaller deleted it already; read again:
    only while it still starts one of ours }
  if TakeOverOldRun and RegQueryStringValue(HKCU, RunKey, OldRunValue, OldRun) and OldRunIsOurs(OldRun) then
    RegDeleteValue(HKCU, RunKey, OldRunValue);
  { 2.0.x's language (now in LanguageKey), unless its uninstaller deleted it }
  RegDeleteValue(HKCU, OldLanguageKey, 'SetupLanguage');
  RegDeleteKeyIfEmpty(HKCU, OldLanguageKey);
  InstallWebView2;
  { An update starts the window, which starts the core }
  if UpdateMode then
    Exec(ExpandConstant('{app}\{#AppExe}'), '', ExpandConstant('{app}'), SW_SHOWNORMAL, ewNoWait, Code)
  else if CoreWasRunning then
    Exec(ExpandConstant('{app}\{#CoreExe}'), '--background', ExpandConstant('{app}'), SW_SHOWNORMAL, ewNoWait, Code);
end;

function HasRecords(const Dir: String): Boolean;
var
  Find: TFindRec;
begin
  Result := FindFirst(Dir + '\*.json', Find);
  if Result then
    FindClose(Find);
end;

{ The adapters BlueCaster put on WinUSB, as the core records them: one record per
  adapter in <settings>\adapters\driver\ (the result of `bluecaster-driver
  assign`, app/src/core/src/driver.rs), deleted on "Restore driver". Before
  3.0.0 they were in <settings>\driver\, and in %APPDATA%\A2DPWB\driver\
  until the app has taken that folder over: the first folder with records.
  Restoring needs the driver tool next to the uninstaller. }
function DriverRecords: String;
var
  Dirs: TArrayOfString;
  I: Integer;
begin
  Dirs := [ConfigDir + '\adapters\driver', ConfigDir + '\driver', ExpandConstant('{userappdata}\A2DPWB\driver')];
  for I := 0 to GetArrayLength(Dirs) - 1 do
    if HasRecords(Dirs[I]) then begin
      Result := Dirs[I];
      Exit;
    end;
  Result := Dirs[0];
end;

function CanRestoreDrivers: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\{#DriverExe}')) and HasRecords(DriverRecords);
end;

{ Adds a choice of the uninstall page below Top, from Left and Width wide:
  a bold check box and the text under it; returns the box, and Top below
  the text }
function AddChoice(Page: TNewNotebookPage; const Caption, Text: String; Checked: Boolean;
  Left, Width: Integer; var Top: Integer): TNewCheckBox;
var
  Hint: TNewStaticText;
begin
  Result := TNewCheckBox.Create(Page);
  Result.Parent := Page;
  Result.Caption := Caption;
  Result.Font.Style := [fsBold];
  Result.Checked := Checked;
  Result.Left := Left;
  Result.Top := Top;
  Result.Width := Width;
  Result.Height := ScaleY(20);
  Hint := TNewStaticText.Create(Page);
  Hint.Parent := Page;
  Hint.AutoSize := False;
  Hint.WordWrap := True;
  Hint.Caption := Text;
  Hint.Left := Left + ScaleX(20);
  Hint.Top := Top + ScaleY(22);
  Hint.Width := Width - ScaleX(20);
  Hint.AdjustHeight;
  Top := Hint.Top + Hint.Height + ScaleY(14);
end;

{ The uninstall's choices on a page of the uninstall wizard itself (issue
  86: a dialog box of its own looked out of place), before anything is
  removed: give the adapters back to Windows (only when BlueCaster put one on
  WinUSB) and delete the user data, both on by default. No word of v1: Setup
  uninstalled it, and the running v2 core holds v1's mutex too (issue #86:
  "v1 is still on this PC" with no v1). The choices line up with the page
  description, as on Setup's pages. Uninstall goes on, Cancel stops the
  uninstall.
  Silent mode shows no page and chooses neither: nobody to ask, and a silent
  uninstall must not raise a UAC prompt. }
procedure InitializeUninstallProgressForm();
var
  Page, ProgressPage: TNewNotebookPage;
  RestoreBox, DeleteBox: TNewCheckBox;
  UninstallButton: TNewButton;
  PageName, PageDescription: String;
  Left, Width, Top: Integer;
begin
  RestoreChosen := False;
  DeleteData := False;
  if UninstallSilent then
    Exit;
  ProgressPage := UninstallProgressForm.InnerNotebook.ActivePage;
  Page := TNewNotebookPage.Create(UninstallProgressForm);
  Page.Notebook := UninstallProgressForm.InnerNotebook;
  Page.Parent := UninstallProgressForm.InnerNotebook;
  Page.Align := alClient;
  { Setup's pages: the content starts under the description and keeps the
    same margin on the right (the notebook here is wider) }
  Left := UninstallProgressForm.PageDescriptionLabel.Left - UninstallProgressForm.InnerNotebook.Left;
  if Left < 0 then
    Left := 0;
  Width := UninstallProgressForm.ClientWidth - 2 * UninstallProgressForm.PageDescriptionLabel.Left;
  if Width > Page.ClientWidth - Left then
    Width := Page.ClientWidth - Left;
  Top := 0;
  RestoreBox := nil;
  if CanRestoreDrivers then
    RestoreBox := AddChoice(Page, CustomMessage('RestoreChoice'), CustomMessage('RestoreChoiceText'), True, Left, Width, Top);
  { Only the settings folder itself, not one BLUECASTER_CONFIG_DIR names }
  DeleteBox := AddChoice(Page, CustomMessage('DeleteChoice'),
    FmtMessage(CustomMessage('DeleteChoiceText'), [ExpandConstant('{userappdata}\BlueCaster')]), True, Left, Width, Top);

  UninstallProgressForm.InnerNotebook.ActivePage := Page;
  PageName := UninstallProgressForm.PageNameLabel.Caption;
  PageDescription := UninstallProgressForm.PageDescriptionLabel.Caption;
  UninstallProgressForm.PageNameLabel.Caption := CustomMessage('UninstallChoices');
  UninstallProgressForm.PageDescriptionLabel.Caption := FmtMessage(CustomMessage('UninstallChoicesLabel'), ['{#AppName}']);

  UninstallButton := TNewButton.Create(UninstallProgressForm);
  UninstallButton.Parent := UninstallProgressForm;
  UninstallButton.Caption := CustomMessage('UninstallButton');
  { Wide enough for its caption ("アンインストール(&U)" was cut off) }
  UninstallButton.Width := UninstallProgressForm.CalculateButtonWidth([CustomMessage('UninstallButton')]);
  if UninstallButton.Width < UninstallProgressForm.CancelButton.Width then
    UninstallButton.Width := UninstallProgressForm.CancelButton.Width;
  UninstallButton.Height := UninstallProgressForm.CancelButton.Height;
  UninstallButton.Top := UninstallProgressForm.CancelButton.Top;
  UninstallButton.Left := UninstallProgressForm.CancelButton.Left - UninstallButton.Width - ScaleX(10);
  UninstallButton.ModalResult := mrOk;
  UninstallButton.Default := True;
  UninstallProgressForm.CancelButton.Enabled := True;
  UninstallProgressForm.CancelButton.ModalResult := mrCancel;
  UninstallProgressForm.ActiveControl := UninstallButton;

  if UninstallProgressForm.ShowModal = mrCancel then
    Abort;

  RestoreChosen := (RestoreBox <> nil) and RestoreBox.Checked;
  DeleteData := DeleteBox.Checked;
  Log(Format('Uninstall choices: restore drivers %d, delete user data %d', [Ord(RestoreChosen), Ord(DeleteData)]));
  { Back to the progress page }
  UninstallButton.Visible := False;
  UninstallProgressForm.CancelButton.Enabled := False;
  UninstallProgressForm.CancelButton.ModalResult := mrNone;
  UninstallProgressForm.InnerNotebook.ActivePage := ProgressPage;
  UninstallProgressForm.PageNameLabel.Caption := PageName;
  UninstallProgressForm.PageDescriptionLabel.Caption := PageDescription;
end;

{ Gives the adapters BlueCaster put on WinUSB back to Windows:
  `bluecaster-driver restore --records` restores every recorded adapter with one
  UAC prompt and deletes the records of those given back. The core must be
  closed already. }
procedure RestoreDrivers;
var
  Records: String;
  Code: Integer;
begin
  Records := DriverRecords;
  Log('Restoring the drivers of the adapters recorded in ' + Records);
  if not ShellExec('runas', ExpandConstant('{app}\{#DriverExe}'), 'restore --records "' + Records + '"', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    { 1223: UAC cancelled }
    Log(Format('%s did not start (%d): the adapters stay on WinUSB', ['{#DriverExe}', Code]))
  { 0: all given back (or nothing to do) }
  else if Code = 0 then
    Log('The adapters are back on their own drivers')
  else begin
    Log(Format('%s restore --records: exit code %d', ['{#DriverExe}', Code]));
    MsgBox(CustomMessage('RestoreFailed'), mbError, MB_OK);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Cmd: String;
begin
  case CurUninstallStep of
    usUninstall:
      begin
        CloseCore(ExpandConstant('{app}'));
        WaitForAppExit;
        if RestoreChosen then
          RestoreDrivers
        else if CanRestoreDrivers then
          Log('The adapters stay on WinUSB');
      end;
    usPostUninstall:
      begin
        { With the window's own data (WebView2: what the pages remember,
          such as "Show only supported codecs", issue #86) }
        if DeleteData then begin
          DelTree(ExpandConstant('{userappdata}\BlueCaster'), True, True, True);
          DelTree(ExpandConstant('{localappdata}\{#AppIdentifier}'), True, True, True);
          { What 2.0.x may have left: its settings (not taken over yet), its
            window's data }
          DelTree(ExpandConstant('{userappdata}\A2DPWB'), True, True, True);
          DelTree(ExpandConstant('{localappdata}\io.github.seiyafunaokajp.a2dpwb'), True, True, True);
        end;
        if RegQueryStringValue(HKCU, RunKey, RunValue, Cmd) and
           (Pos(Lowercase(ExpandConstant('{app}\{#CoreExe}')), Lowercase(Cmd)) > 0) then
          RegDeleteValue(HKCU, RunKey, RunValue);
      end;
  end;
end;
