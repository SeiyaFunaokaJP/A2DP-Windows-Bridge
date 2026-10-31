---
title: Instalación
layout: default
parent: Inicio
nav_order: 1
---

# Instalación
{: .no_toc }

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Requisitos previos

- **Windows 10 (22H2) / 11** (x64)
- **Un adaptador Bluetooth USB para BlueCaster**: se recomienda un dongle aparte; un adaptador USB integrado también sirve, pero Windows se queda sin Bluetooth propio (consulte el [Paso 2](#winusb))

BlueCaster asigna por sí mismo el controlador WinUSB al adaptador. [Zadig](https://zadig.akeo.ie/) ya no es necesario (sigue funcionando; consulte [Usar Zadig en su lugar](#zadig)).

## Instalar BlueCaster

Cada [versión](https://github.com/SeiyaFunaokaJP/BlueCaster/releases/latest) se publica en dos formatos con los mismos archivos:

- **Instalador** (`BlueCaster-<version>-win64-setup.exe`): instala para el usuario actual sin derechos de administrador, añade una entrada en el menú Inicio (**BlueCaster**) y un desinstalador, e instala Microsoft Edge WebView2 Runtime si falta (Windows 11 ya lo incluye). Las actualizaciones se instalan solas desde la aplicación (**Configuración > Actualizaciones > Actualizar ahora**)
- **Portable** (`BlueCaster-<version>-win64-portable.zip`): extráigalo en cualquier carpeta y ejecute `bluecaster.exe`. Mantenga juntos los tres programas (`bluecaster.exe`, `bluecaster-core.exe`, `bluecaster-driver.exe`). La copia portable solo avisa de las actualizaciones; necesita WebView2 Runtime instalado

Ambos guardan la configuración, los perfiles, las claves de enlace y el firmware en `%APPDATA%\BlueCaster`, así que puede cambiar de uno a otro. La desinstalación no elimina esa carpeta. Los programas están en `%LOCALAPPDATA%\BlueCaster\` y los textos de licencia, en la carpeta `licenses\`.

### Actualizar desde la v1 {#from-v1}

Ejecute el instalador de la v2 sobre la v1. Si la v1 está en ejecución, le pide que la cierre; después desinstala la v1 y conserva la carpeta de configuración: los perfiles, el emparejamiento con los auriculares y los archivos de firmware se mantienen, y no hace falta volver a emparejar los auriculares. Un adaptador que ya tiene WinUSB (instalado con Zadig) lo conserva. Si **Iniciar con Windows** estaba activado, ahora inicia la v2.

En su primer inicio, la v2 copia la configuración de la v1 en `%APPDATA%\BlueCaster\backup\v1\` antes de leerla.

### Actualizar desde la 2.0.x (A2DP Windows Bridge) {#from-2x}

BlueCaster 3.0.0 es el nuevo nombre de A2DP Windows Bridge. Ejecute el instalador de la 3.0.0 sobre la 2.0.x instalada: el instalador desinstala solo la versión anterior y conserva la configuración. La entrada del menú Inicio y la de **Iniciar con Windows** pasan a «BlueCaster».

En el primer inicio, la 3.0.0 cambia el nombre de la carpeta `%APPDATA%\A2DPWB` a `%APPDATA%\BlueCaster` y ordena los archivos en subcarpetas (véase [Configuración y archivos](../architecture#settings-and-files)). `backup\v2\` contiene una copia de la configuración y los emparejamientos de la 2.0.x, para poder volver a instalar la 2.0.2: copie esos archivos de nuevo en `%APPDATA%\A2DPWB`.

Los auriculares pueden seguir mostrando el antiguo nombre Bluetooth «A2DPWB» hasta que se vuelvan a emparejar.

## Paso 1: Configuración inicial {#first-run}

Conecte el adaptador Bluetooth USB e inicie BlueCaster. Si aún no hay ningún perfil o ningún adaptador tiene WinUSB, BlueCaster abre **Configurar BlueCaster**, con tres pasos:

1. **Elegir el adaptador**: los adaptadores Bluetooth USB encontrados. Si falta el suyo, conéctelo y pulse **Actualizar**. Un **Compatible** verde o un **No compatible** rojo indica si BlueCaster puede usarlo; uno no compatible aparece en la lista, pero no se puede elegir
2. **Controlador y firmware**: **Reemplazar por el controlador WinUSB** (consulte el [Paso 2](#winusb)) y, en los adaptadores Realtek, los archivos de firmware (consulte [Firmware](#firmware))
3. **Encontrar los dispositivos de audio**: la búsqueda empieza sola. Ponga el dispositivo de audio en modo de emparejamiento y pulse **Conectar** junto a él (o escriba su dirección y pulse **Usar**): BlueCaster crea su perfil («*nombre* | Auto», el mejor códec del dispositivo) y se conecta; el dispositivo sale de la lista (también después de **Atrás** y **Siguiente**). Después, **Finalizar** (consulte el [Paso 3](#headphones)). El bloque **Inicio** de este paso tiene **Iniciar con Windows** e **Iniciar en la bandeja**, los mismos interruptores que en Configuración, que se guardan al instante

**Configurar más tarde** omite el resto; **Configuración > Ejecutar la configuración inicial** la vuelve a abrir.

## Paso 2: Asignar WinUSB al adaptador {#winusb}

BlueCaster se comunica directamente con un adaptador Bluetooth USB mediante WinUSB, el controlador USB genérico incluido en Windows. Se recomienda un **adaptador dedicado**: así el Bluetooth integrado sigue funcionando con normalidad para el Bluetooth habitual de Windows.

1. Abra la pantalla **Adaptador** (o el paso **Controlador y firmware** de la configuración inicial)
2. El **Controlador** del adaptador indica **Controlador original** (o **Sin controlador**): el controlador que instaló Windows o el fabricante, que WinUSB reemplaza. Haga clic en **Reemplazar por el controlador WinUSB**
3. Lea la confirmación y continúe: Windows solicita permiso de administrador (UAC)
4. El controlador indica ahora **Controlador WinUSB**

Si Windows no pudo cargar WinUSB en ese momento (el adaptador estaba en uso), BlueCaster lo indica y la línea del controlador muestra **Reconecta o reinicia Windows**. Desconecte el adaptador y vuelva a conectarlo; si es un adaptador integrado, reinicie Windows. Hasta entonces, la conexión falla con `failed to initialize WinUSB (error 50)` en el registro.

{: .warning }
Mientras WinUSB esté asignado, el Bluetooth de Windows ya no ve el adaptador: los dispositivos emparejados con Windows a través de él dejan de funcionar hasta que se restaure el controlador. Use el Bluetooth integrado para los periféricos habituales (teclado, ratón, etc.). Un adaptador integrado que sea un dispositivo USB (uno de Realtek en muchos portátiles) también puede cederse a BlueCaster, pero entonces **el PC se queda sin Bluetooth de Windows** (configuración de Bluetooth, ratones, teclados, otros dispositivos de audio) hasta que restaure su controlador.

**Restaurar el controlador original**, en la misma pantalla, devuelve el adaptador a Windows: Windows vuelve a elegir su propio controlador (normalmente su controlador Bluetooth) y el adaptador funciona con el Bluetooth de Windows. BlueCaster no podrá transmitir a través de él hasta que se asigne WinUSB de nuevo. El desinstalador ofrece lo mismo: si BlueCaster asignó WinUSB a algún adaptador, pregunta si devolverlo a Windows (una sola solicitud de permisos de administrador). Las actualizaciones mantienen WinUSB, igual que una desinstalación silenciosa (`/S`).

### Usar Zadig en su lugar {#zadig}

Un controlador WinUSB instalado con [Zadig](https://zadig.akeo.ie/) funciona igual de bien:

1. Abra Zadig y vaya a **Options > List All Devices**
2. Seleccione su adaptador Bluetooth USB en la lista desplegable
3. Seleccione **WinUSB** como controlador de destino y haga clic en **Replace Driver**

### A mano en el Administrador de dispositivos {#device-manager}

Si el adaptador no aparece en la lista de BlueCaster, falla un botón de la pantalla **Adaptador** o su adaptador se comporta de forma distinta a otros de su tipo, el Administrador de dispositivos de Windows (clic derecho en **Inicio** > **Administrador de dispositivos**) hace lo mismo a mano. Ambas cosas requieren permisos de administrador. Los nombres de los menús pueden variar un poco según la versión de Windows.

**Reemplazar por el controlador WinUSB**

1. Busque el adaptador: normalmente en **Bluetooth** con su propio nombre (por ejemplo «TP-Link Bluetooth USB Adapter»), o con una marca amarilla cuando su controlador no pudo iniciarse. **Ver > Dispositivos por conexión** ayuda a distinguirlo del Bluetooth integrado del equipo, que no debe tocar salvo que quiera cederlo a BlueCaster
2. Clic derecho > **Actualizar controlador** > **Buscar controladores en mi equipo** > **Elegir en una lista de controladores disponibles en el equipo**
3. Si Windows pide un tipo de dispositivo, elija **Dispositivos de bus serie universal**. Desmarque **Mostrar hardware compatible**, elija el fabricante **Dispositivo WinUsb** y el modelo **Dispositivo WinUsb**, haga clic en **Siguiente** y confirme la advertencia
4. El adaptador aparece ahora como **Dispositivo WinUsb** en **Dispositivos de bus serie universal**. Reinicie BlueCaster (o vuelva a abrir la pantalla **Adaptador**): lo usa igual que con Zadig

**Restaurar el controlador original**

1. En **Dispositivos de bus serie universal**, clic derecho en el adaptador (**Dispositivo WinUsb**) > **Desinstalar el dispositivo**. Si el controlador WinUSB vino de Zadig, marque también **Intentar quitar el controlador de este dispositivo**; si no, Windows vuelve a elegir el controlador de Zadig
2. Desconecte el adaptador y vuelva a conectarlo (o **Acción > Buscar cambios de hardware**). Windows instala su propio controlador y el adaptador aparece en **Bluetooth**
3. Si ahí muestra una marca amarilla (su controlador no pudo iniciarse, por ejemplo código 31), el controlador del fabricante no funciona con él: clic derecho > **Actualizar controlador** > **Buscar controladores en mi equipo** > **Elegir en una lista de controladores disponibles en el equipo** > **Adaptador Bluetooth genérico**, o asigne de nuevo WinUSB como arriba para usarlo con BlueCaster

## Paso 3: Encontrar los auriculares {#headphones}

**Buscar dispositivos** (en la configuración inicial y en **Dispositivos de audio**) busca dispositivos cercanos con el adaptador USB durante unos 10 segundos. Ponga antes los auriculares en modo de emparejamiento. Los dispositivos con los que BlueCaster ya se ha emparejado aparecen como **Emparejado**.

Si la búsqueda no los encuentra, escriba la dirección Bluetooth en **Manual:** y haga clic en **Usar**. Las direcciones escritas o pegadas se formatean mientras escribe: `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` y `AA BB CC DD EE FF` se convierten todas en `AA:BB:CC:DD:EE:FF`.

**Desde la Configuración de Windows** (si los auriculares se emparejaron con el Bluetooth integrado del PC):
1. Abra **Configuración > Bluetooth y dispositivos**
2. Haga clic en su dispositivo de audio
3. Haga clic en **Propiedades**
4. Se muestra la dirección Bluetooth (formato: `AA:BB:CC:DD:EE:FF`)

**Desde la línea de comandos** (cierre antes BlueCaster, bandeja > **Salir**):
```
bluecaster-core.exe --cli --scan
```

Los auriculares empiezan con **Automático** como códec (**Códecs**) y el audio del sistema como origen de salida (**Dispositivos de audio**); los valores predeterminados dan el mejor sonido. Consulte [Uso](usage) para todo lo demás.

---

## Firmware (adaptadores Realtek) {#firmware}

Los adaptadores Bluetooth USB basados en Realtek (p. ej., TP-Link UB500, llaves RTL8761BU) necesitan firmware propietario para funcionar. Los adaptadores CSR no necesitan firmware. Los adaptadores Intel y Broadcom (experimentales) pueden necesitar sus propios archivos de firmware: consulte [Adaptadores recomendados](#adapters).

### En la aplicación (descarga guiada)

La pantalla **Adaptador** (y el paso **Controlador y firmware** de la configuración inicial) muestra el firmware del adaptador: **Firmware correcto**, **No necesita archivos de firmware** o **Firmware no encontrado** con los nombres de los archivos necesarios. Use el botón **Abrir página de descarga** para abrir la página [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) en el navegador y, después, **Abrir carpeta de configuración** para abrir la carpeta de destino. Descargue los archivos `.bin` necesarios y colóquelos en la carpeta de configuración.

Para los chips de la lista **Chip Bluetooth**, **Descargar firmware** lo hace por usted: tras confirmar, BlueCaster descarga los archivos de linux-firmware (kernel.org), comprueba cada uno con el SHA-256 esperado y lo guarda en la carpeta de configuración. Un chip para el que linux-firmware no tiene archivo config recibe un `_config.bin` vacío (el chip usa entonces su propia configuración). Los archivos no forman parte de BlueCaster; solo se descargan cuando usted lo pide.

BlueCaster detecta el chipset del adaptador y muestra qué archivos de firmware se necesitan.

Sin el firmware, un adaptador Realtek se abre igualmente y responde con su código de ROM, así que parece correcto, pero la conexión o la transmisión fallan. BlueCaster indica entonces que el firmware no está cargado. Un módulo conserva el firmware cargado en una sesión anterior hasta que pierde la alimentación, así que, sin los archivos, puede seguir funcionando hasta que se reinicie el PC o se desconecte el adaptador.

### Opción Chip Bluetooth

La lista **Chip Bluetooth** de la pantalla **Adaptador** decide si BlueCaster ejecuta la inicialización de Realtek (carga del firmware) en el adaptador:

| Opción | Significado |
|:-------|:--------|
| **Automático** (predeterminado) | Se decide según el adaptador detectado cada vez que se inicia BlueCaster. Un adaptador Realtek conocido recibe su chip; cualquier otro adaptador no recibe inicialización de Realtek |
| **No Realtek** | No ejecutar nunca la inicialización de Realtek. Úsela para adaptadores Intel, Broadcom, CSR y otros si Automático no reconoce el suyo |
| **RTL8761BU**, **RTL8822CU**, ... | Ese chip Realtek. Necesario para un adaptador basado en Realtek que Automático no reconoce (un producto OEM con su propio VID:PID). La línea de estado indica si sus archivos `.bin` están en la carpeta de configuración |
| **Custom** | Usa `custom_fw.bin` y `custom_config.bin` de la carpeta de configuración |

No elija un chip Realtek ni Custom para un adaptador que no sea Realtek: BlueCaster intentaría cargarle firmware de Realtek y es posible que el adaptador no se inicie.

### Descarga manual

1. Descargue los archivos `.bin` de firmware y de configuración de su chipset desde [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) (p. ej., `rtl8761bu_fw.bin` y `rtl8761bu_config.bin`)
2. Coloque ambos archivos en la carpeta de configuración de BlueCaster (`%APPDATA%\BlueCaster\firmware`, también en **Configuración > Carpeta de configuración**)

{: .note }
Estos archivos de firmware son binarios propietarios de Realtek distribuidos a través del proyecto linux-firmware. No se incluyen en este repositorio. Consulte [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) para conocer las condiciones de redistribución.

---

## Adaptadores recomendados {#adapters}

| Chipset | Productos de ejemplo | Notas |
|:--------|:-----------------|:------|
| Realtek | TP-Link UB500, RTL8761BU | Requiere descargar firmware (ver arriba). El más probado |
| CSR | Llaves CSR8510 genéricas | Funcionan sin más, no necesitan firmware |
| Broadcom | ASUS BT400 (BCM20702) | *Experimental.* También se detectan los adaptadores que indican la clase de fabricante `FF/01/01` en lugar de la clase Bluetooth `E0/01/01` (llaves OEM BCM20702). Funciona con el firmware en ROM; un archivo PatchRAM `.hcd` opcional (p. ej. `BCM20702A1-0b05-17cb.hcd`) en la carpeta de configuración se carga al iniciar |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *Experimental.* En modo bootloader (p. ej. tras un arranque en frío) necesita los archivos `ibt-*.sfi` + `ibt-*.ddc` correspondientes de `intel/` de linux-firmware en la carpeta de configuración. AX210 y posteriores (bootloader TLV) aún no son compatibles |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | No compatibles: necesitan cargadores de firmware que BlueCaster no tiene |

Por lo general, los adaptadores USB Bluetooth 5.0+ funcionan mejor con códecs de alta tasa de bits como LDAC.

Si varios adaptadores tienen WinUSB, BlueCaster usa el primero que encuentra. Mantenga WinUSB solo en el adaptador que usa con BlueCaster.
