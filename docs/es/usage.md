---
title: Uso
layout: default
parent: Inicio
nav_order: 2
---

# Uso
{: .no_toc }

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## La ventana

Inicie **A2DP Windows Bridge** desde el menú Inicio (o `a2dpwb.exe`). El menú de la izquierda tiene las pantallas siguientes (Inicio, Códecs, Dispositivos de audio, Adaptador, Diagnóstico, Configuración, Acerca de); se reduce a iconos cuando la ventana es estrecha. La ventana solo muestra lo que hace el programa residente `a2dpwb-core.exe`: cerrarla no detiene la transmisión (consulte [Bandeja y panel desplegable](#tray)).

### Inicio

- Una tarjeta para cada dispositivo de audio guardado, una junto a otra: su estado, la batería y, en el conectado, el códec en uso (una lista en lugar de su nombre, véase más abajo) con su tasa de bits. Cuando se usa un códec distinto del elegido, la insignia del códec indica cuál y por qué durante unos segundos; después solo queda su marca de advertencia (al pasar el puntero se ve el motivo)
- **Conectar** / **Desconectar** bajo cada tarjeta. Solo hay un dispositivo conectado a la vez: **Conectar** en otro dispositivo desconecta el que está en uso y conecta ese
- **Volumen**: el volumen de los auriculares (consulte [Volumen](#volume)); las tarjetas de los demás dispositivos conservan el control deslizante, desactivado
- El códec del dispositivo en uso, elegido en la lista de su tarjeta para cambiarlo rápido (por ejemplo, uno para juegos y otro para música): los códecs que el dispositivo admite, como en [Códecs](#codecs), solo por su nombre. Muestra el códec en uso, también con **Selección automática**; elegir un códec desactiva la selección automática (vuelva a activarla en Códecs). La elección se aplica al instante (el dispositivo se vuelve a conectar con él; la siguiente elección espera hasta que esté conectado, como en Códecs)
- Para el dispositivo en uso, la calidad del enlace con un pequeño gráfico, debajo de las tarjetas. El origen de captura se elige en **Dispositivos de audio**; mientras una aplicación establecida como origen de captura aún no se está ejecutando, la tarjeta del dispositivo lo indica
- Un error aparece con su causa y, si lo hay, el botón que lo soluciona (**Reemplazar por el controlador WinUSB**, **Abrir página de descarga**, **Reiniciar Bluetooth**, **Reintentar**, **Códecs**, **Dispositivos de audio**, ...)

La barra lateral muestra lo mismo para cada dispositivo en todas las pantallas.

### Códecs {#codecs}

El códec de cada dispositivo de audio guardado, una sección por dispositivo (nombre, etiquetas **Emparejado** / **Audio**; la dirección está en **Dispositivos de audio**) con sus tarjetas de códec: **Automático** (el mejor códec que admite el dispositivo) y los códecs que el dispositivo ofreció en su última conexión, cada uno con tres barras (calidad de sonido, baja latencia, estabilidad). Al hacer clic en una tarjeta se elige el códec de ese dispositivo; se guarda al instante, y un dispositivo que está transmitiendo vuelve a conectarse con él.

- Elegir otro códec para el dispositivo que está transmitiendo lo vuelve a conectar con ese códec. Después, las tarjetas (y la lista de códecs de **Inicio** y del panel desplegable de la bandeja) esperan hasta que vuelva a estar conectado (**Cambiando el códec. Espera un momento antes de elegir otro.**, al menos 1 segundo y como mucho 15) antes de la siguiente elección, para que una serie rápida de clics no interrumpa una conexión que aún se está estableciendo. Cambiar de códec no muestra ninguna notificación de conexión; conectar un dispositivo sí
- **Mostrar solo los códecs compatibles** (activado de forma predeterminada; desactívelo para mostrar los códecs compatibles y los no compatibles): se omiten los códecs que un dispositivo no ofreció (el número de ocultos aparece entre paréntesis). Antes de la primera conexión de un dispositivo se muestran todos
- **Seleccionar automáticamente el códec compatible de mayor calidad** (un ajuste para todos los dispositivos, arriba en la página; activado de forma predeterminada): cada dispositivo usa el códec compatible de mayor calidad de sonido, con sus mejores ajustes: alta calidad, 24 bits si el códec los admite y la frecuencia de muestreo más alta que anuncia el dispositivo (p. ej., LDAC a 96 kHz, aptX Adaptive a 96 kHz en un dispositivo R2, LHDC V5 hasta 192 kHz; la frecuencia de Windows se remuestrea); las tarjetas son de solo lectura y muestran el códec en uso (o el que elegirá la selección automática). Desactívelo para elegir el códec de cada dispositivo y sus ajustes (un dispositivo sin elección conserva el de la selección automática). El códec elegido para un dispositivo se conserva mientras está activado. Si la configuración anterior a 2.0.2 elegía un códec para algún dispositivo, empieza desactivado
- **Orden de la selección automática…** (debajo del interruptor): abre la lista de códecs en el orden en que los elige la selección automática (de forma predeterminada, mayor calidad de sonido primero). Arrastre un códec, o use sus flechas, para ordenarlos; **Restablecer el orden de los códecs** lo restablece. La selección automática usa el primer códec de la lista que admita el dispositivo, y un códec elegido no admitido pasa al siguiente por debajo en este orden. Las tarjetas siguen este orden. Si cambia el códec de una transmisión en curso, se vuelve a conectar
- **Activar códecs experimentales** (activado de forma predeterminada): los códecs experimentales (aptX HD, aptX Low Latency, Opus, Opus 05, FastStream) llevan la etiqueta **Experimental** en sus tarjetas. Si se desactiva, sus tarjetas se ocultan, Automático nunca los elige y un dispositivo cuyo códec elegido sea uno de ellos usa el siguiente códec inferior en el orden de Automático que ofrezca el dispositivo; la página lo indica. Si esto cambia el códec de una transmisión en curso, el dispositivo vuelve a conectarse. CLI: `--no-experimental`
- **Usar otro códec compatible si este no está disponible** (activado de forma predeterminada, para todos los dispositivos; consulte [El códec seleccionado no está disponible](#codec-fallback))
- **...** en una tarjeta abre los ajustes de ese códec; **Aceptar** elige el códec con ellos. Cada códec conserva sus propios ajustes: si elige otro códec y vuelve, se recuperan. Solo se ofrece lo que ese códec admite:
  - **Calidad**: LDAC HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps); LHDC V5 Alta (1000 kbps) / Media-alta (900 kbps) / Estándar (500 kbps) / Móvil (400 kbps), los cuatro niveles que ofrece Android; aptX Adaptive Alta / Estándar / Móvil / Baja latencia (consulte [aptX Adaptive](#aptx-adaptive)); AAC también tiene niveles de calidad, los demás códecs aptX una tasa de bits fija. SBC y SBC XQ muestran la tasa de bits que resulta de su bitpool y de la configuración de tramas
  - **ABR (tasa de bits adaptativa)**: para LDAC, LHDC V5, aptX Adaptive, SBC y SBC XQ (consulte [ABR](#abr)); **VBR** para AAC
  - **Frecuencia de muestreo**: Automático es 48 kHz, o la mejor que anuncie el dispositivo si no tiene 48 kHz (96 kHz añade latencia y problemas de compatibilidad sin una mejora audible, y Windows mezcla a 48 kHz); elija 96 kHz aquí para usarlo
  - **Profundidad de bits**: 16 o 24 bits para LDAC y LHDC V5 (Automático: 24 bits); 16 bits para los demás

Los valores predeterminados dan el mejor sonido; la mayoría de los dispositivos no necesitan más que **Automático**. Un dispositivo LE Audio (experimental) tiene una tarjeta, LC3, y **Baja latencia para auriculares gaming (GMAP, experimental)** (consulte [LE Audio](#le-audio)).

Por debajo, A2DPWB guarda un perfil por dispositivo en `profiles.json`, llamado allí «*dispositivo* | *códec*»; la ventana y el menú de la bandeja muestran el nombre del dispositivo. Las versiones anteriores permitían crear varios perfiles para un mismo dispositivo: en el primer inicio de una versión con la pantalla Códecs se conserva el último usado (si no, el primero) y los demás se integran en él, manteniendo sus ajustes de códec (calidad, frecuencia de muestreo, profundidad de bits, ABR, configuración SBC) para sus códecs. No se conservan sus nombres, orígenes de captura ni tamaños de paquete; el `profiles.json` tal como estaba se guarda una vez como `profiles-before-codecs.json` en la carpeta de configuración.

### Dispositivos de audio

Los auriculares y altavoces a los que A2DPWB puede transmitir, cada uno con sus propios ajustes: **Buscar dispositivos** busca dispositivos cercanos con el adaptador USB (unos 10 segundos; ponga los auriculares en modo de emparejamiento). Los dispositivos con los que A2DPWB ya se ha emparejado llevan la etiqueta **Emparejado**. Si la búsqueda no los encuentra, escriba la dirección en **Manual:** y haga clic en **Usar**; las direcciones escritas o pegadas se formatean mientras escribe (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` y `AA BB CC DD EE FF` se convierten todas en `AA:BB:CC:DD:EE:FF`). Un dispositivo guardado se puede renombrar o eliminar, y tiene:

- **Origen de captura**: Loopback del sistema (el predeterminado de un dispositivo nuevo), Dispositivo virtual o Aplicación (consulte [Modos de captura](#capture-modes)). A su lado, el formato en que Windows mezcla el dispositivo capturado (frecuencia de muestreo y profundidad de bits, de solo lectura), en ámbar cuando los ajustes de códec del dispositivo piden otro, con una línea debajo que indica cuál (sigue transmitiendo, convertido); debajo, lo que hace el origen
- **Cambiar automáticamente la salida predeterminada de Windows**, con un dispositivo virtual

**Todos los dispositivos de audio**, encima de la lista, tiene los interruptores que se aplican a todos los dispositivos: **Sincronizar volumen de Windows** (consulte [Volumen](#volume)), **Control de reproducción con los botones del dispositivo de audio** y **Recordar el último volumen del dispositivo de audio Bluetooth**. El **Tamaño máx. de paquete multimedia** de cada dispositivo está en la pantalla **Desarrollador** (consulte [Tamaño máximo del paquete multimedia](#max-packet)).

### Diagnóstico

- **Calidad del enlace** durante la transmisión: tasa de bits real, paquetes, paquetes descartados antes del envío, audio descartado en la captura, cola de envío, tiempo de radio usado, señal (RSSI) y los canales AFH (consulte [Calidad del enlace y AFH](#link-quality))
- **Detalles del códec** durante la transmisión: la carga útil multimedia, los encabezados, la trama del códec y las tramas por paquete, y lo que anuncia el endpoint del dispositivo junto a lo que se usa: en SBC el bitpool, las frecuencias de muestreo, los modos de canal, los bloques, las subbandas y la asignación de bits; en AAC los tipos de objeto, las frecuencias de muestreo, los canales, la tasa de bits máxima y VBR; en los demás códecs las frecuencias de muestreo, las profundidades de bits y la tasa de bits máxima
- **Flujo de conexión**: las once etapas del último intento de conexión (inicio, adaptador Bluetooth, conexión ACL / AVDTP, detección de capacidades, selección de códec, captura de audio, configuración de la transmisión, codificador, inicio de la transmisión, envío de audio, mantenimiento de la conexión con desconexiones / reconexiones) como OK, Advertencia o Error, con el tiempo que tardó cada una. **Copiar flujo** lo copia
- **Registro**: en directo, filtrado por nivel (todos, info y superiores, advertencias y errores, solo errores) y por texto. **Copiar registro visible** copia lo que se muestra
- **Iniciar captura HCI** / **Detener captura HCI** (consulte [Verificar el flujo multimedia](#verify-stream))
- **Copiar diagnóstico**: la versión, el sistema operativo, el adaptador, el perfil, el flujo, las advertencias / errores y el registro reciente al portapapeles, listo para pegar en un informe de errores. **Ocultar direcciones Bluetooth** (activado de forma predeterminada) sustituye las direcciones por `<addr-1>`, `<addr-2>`... (la del adaptador por `<adapter>`) y los nombres de dispositivo por `<device-1>`, `<device-2>`..., y oculta los números de serie USB y su nombre de usuario de Windows

Con **Configuración > Modo de desarrollador**, Diagnóstico tiene además una pestaña **Prueba con receptor par** (consulte [Prueba con receptor par](#receiver)).

### Adaptador

- Los adaptadores Bluetooth USB detectados con su chip, **Controlador** y **Firmware**; la lista se actualiza sola al conectar o desconectar un adaptador. Un adaptador que A2DPWB no puede usar (MediaTek, Qualcomm) conserva su controlador: sus botones de WinUSB y de firmware están desactivados
- **Reemplazar por el controlador WinUSB** / **Restaurar el controlador original** (consulte [Instalación](setup#winusb)); Windows solicita permiso de administrador cada vez
- **Chip Bluetooth** para el firmware, con **Descargar firmware**, **Abrir página de descarga** y **Abrir carpeta de configuración** (consulte [Firmware](setup#firmware))
- **AFH**: Automático, Desactivado o Manual, con los canales Wi-Fi al lado: los que Automático evita ahora y ninguno con Desactivado (ambos de solo lectura), los que escriba con Manual (consulte [Calidad del enlace y AFH](#link-quality))
- **Pila Bluetooth**: detenida (se inicia con la primera búsqueda o conexión), iniciando, lista o error, y **Reiniciar Bluetooth**

### Configuración

| Sección | Ajustes |
|:--------|:---------|
| Apariencia | Idioma, tema (Sistema / Claro / Oscuro) |
| Inicio | **Iniciar con Windows** e **Iniciar en la bandeja** (consulte [Iniciar con Windows](#autostart)), **Minimizar en la bandeja del sistema** y **Al hacer clic en el icono de la bandeja se abre la ventana principal** (consulte [Bandeja y panel desplegable](#tray)) |
| Notificaciones | **Notificar cuando se conecta un dispositivo** (el dispositivo, su batería, el códec y con qué funciona; activado de forma predeterminada), **Avisar cuando la batería esté baja**: **Por debajo del 20%** (el predeterminado), **Por debajo del 10%**, **Por debajo del 5%** o **Nunca**, una vez por conexión (de nuevo solo después de que la batería haya vuelto a estar 5 puntos por encima) |
| Actualizaciones | **Buscar actualizaciones**, **Buscar actualizaciones al iniciar**, **Incluir versiones preliminares al buscar actualizaciones** (también ofrece versiones beta), **Actualizar ahora** (consulte [Actualizaciones](#updates)) |
| Avanzado | **Modo de desarrollador** (añade la pantalla **Desarrollador** con el **Tamaño máx. de paquete multimedia** de cada dispositivo, la prueba con receptor a Diagnóstico y más detalles al registro; no requiere reiniciar), **Carpeta de configuración** con **Abrir carpeta de configuración**, **Ejecutar la configuración inicial** |
| Acerca de A2DPWB | Versión, licencia, licencias de terceros, **Salir de A2DPWB** |

### Configuración inicial

El primer inicio (aún sin perfil, o sin ningún adaptador con WinUSB) abre **Configurar A2DPWB**: elegir el adaptador, controlador y firmware, encontrar los dispositivos de audio («Conectar» crea el perfil y se conecta; este paso también tiene **Iniciar con Windows** e **Iniciar en la bandeja**). Consulte [Instalación](setup#first-run). **Configuración > Ejecutar la configuración inicial** la vuelve a abrir.

## Bandeja y panel desplegable {#tray}

A2DPWB sigue ejecutándose en la bandeja del sistema, con o sin la ventana:

- **Clic izquierdo** en el icono de la bandeja abre el panel desplegable: el dispositivo y su estado, **Conectar** / **Desconectar**, el volumen, el selector de dispositivos, el códec (una lista para elegirlo, como en Inicio), la calidad del enlace y **Abrir la aplicación**. Su altura se ajusta exactamente a lo que muestra. Esc o un clic en otro sitio lo ocultan. Si un error requiere una pantalla de la ventana principal (Códecs, Dispositivos de audio, el adaptador), su botón abre la ventana en esa pantalla. Con **Configuración > Al hacer clic en el icono de la bandeja se abre la ventana principal** activado (desactivado de forma predeterminada), un clic izquierdo abre en su lugar la ventana principal
- **Clic derecho** abre el menú: **Abrir A2DPWB**, los 10 primeros dispositivos por su nombre (una entrada LE Audio marcada con «(LE Audio)»; el que transmite, marcado; haga clic en uno para conectarlo), **Desconectar** y **Salir**. El menú se abre al instante, sin iniciar la ventana
- La información sobre herramientas indica si A2DPWB está inactivo o conectado

Con **Configuración > Minimizar en la bandeja del sistema** activado (el valor predeterminado), minimizar o cerrar la ventana solo la oculta en la bandeja, y la transmisión continúa. Si está desactivado, la ventana se minimiza en la barra de tareas y cerrarla sale de A2DPWB. **Salir** en el menú de la bandeja o **Configuración > Salir de A2DPWB** detiene la transmisión, libera el adaptador y cierra A2DPWB por completo.

## Iniciar con Windows {#autostart}

**Configuración > Iniciar con Windows** inicia A2DPWB al iniciar sesión. Con **Iniciar en la bandeja** activado, se inicia en segundo plano: solo aparece el icono de la bandeja, y la ventana se abre desde él. Si está desactivado (el valor predeterminado), se abre la ventana principal. Es el valor `A2DPWB` en `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, con los datos `"<folder>\a2dpwb-core.exe"`, seguidos de `--background` cuando **Iniciar en la bandeja** está activado. Para desactivarlo a mano, elimine ese valor. Al instalar la v2 sobre la v1, **Iniciar en la bandeja** queda activado, ya que la v1 siempre se iniciaba en la bandeja.

## Modo CLI

`a2dpwb-core.exe --cli` transmite desde una consola, con el mismo núcleo que la ventana. Cierre antes A2DPWB (bandeja > **Salir**): solo un programa a la vez puede usar el adaptador. La CLI no lee los perfiles; todo se indica como opciones.

### Básico

```bash
# Seleccionar automáticamente el mejor códec
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C detiene la transmisión. La CLI muestra el estado, el códec y la tasa de bits y, al final, el número de paquetes enviados y descartados.

### Selección de códec

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (experimental; 48 kHz, 256 kbps)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # Opus 05 de PipeWire (experimental; hacia receptores PipeWire)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz, --rate 96000 para 96 kHz)
```

Prioridad de la selección automática: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream y Opus 05, ambos experimentales, solo si se eligen)

Si los auriculares no ofrecen el códec solicitado, la CLI se detiene con un error que explica el motivo; use `-c auto` (el valor predeterminado) para tomar el mejor códec que ofrezcan; consulte [Familia aptX y aptX Adaptive](#aptx-compatibility).

### Calidad

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (predeterminado)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# LHDC V5: hq 1000 / sq 500 / mq 400 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # 500 kbps

# aptX Adaptive: hq 427 / sq 364 / mq 318 / ll 265 kbps (receptores R1)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Baja latencia
```

### ABR (tasa de bits adaptativa) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

ABR reduce automáticamente la tasa de bits cuando la conexión Bluetooth es inestable y la vuelve a subir cuando las condiciones mejoran. LDAC cambia entre sus modos de calidad; LHDC V5 sigue la política de Android y se mantiene entre 160 y 400 kbps; aptX Adaptive pasa de un nivel a otro en Alta (consulte [aptX Adaptive](#aptx-adaptive)). En la ventana, ABR está en los ajustes de un códec (**...** en su tarjeta en **Códecs**).

### Detección de dispositivos

```bash
# Dispositivos cercanos, encontrados con el adaptador USB (10 segundos; --scan 20 para más tiempo)
a2dpwb-core.exe --cli --scan
```

Ponga antes los auriculares en modo de emparejamiento.

### Todas las opciones {#all-options}

| Opción | Descripción |
|:-------|:------------|
| `-d <addr>` | Dirección Bluetooth de los auriculares / del receptor (`AA:BB:CC:DD:EE:FF`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aac`, `sbc`, `lhdcv5`, `opus`, `opus05`, `aptxad`, `auto` (predeterminado: `auto`) |
| `-q <mode>` | Calidad: `hq` (predeterminado), `sq`, `mq`; aptX Adaptive también `ll` (Baja latencia; los demás códecs lo tratan como `hq`) |
| `-a` | ABR (LDAC, LHDC V5 y aptX Adaptive) |
| `--no-experimental` | No usar nunca los códecs experimentales (aptX HD, aptX LL, Opus, Opus 05, FastStream), como con **Códecs > Activar códecs experimentales** desactivado |
| `-m <mode>` | Modo de captura: `loopback` (predeterminado), `virtual`, `app` |
| `--audio-device <id>` | Dispositivo de reproducción para `-m virtual` |
| `--app <exe>` | Ejecutable para `-m app` (p. ej. `spotify.exe`) |
| `--rate <hz>` | Frecuencia de muestreo de captura (predeterminado: la del dispositivo) |
| `--bits <16\|24>` | Profundidad de bits (predeterminado: 24 si el códec la admite) |
| `--max-packet <bytes>` | Tamaño máximo del paquete multimedia, 679-1679, predeterminado 1023 (consulte [Tamaño máximo del paquete multimedia](#max-packet)) |
| `--afh <auto\|off\|6,11>` | Clasificación de canales AFH del host (consulte [Calidad del enlace y AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Mostrar lo que recibe el receptor par; sin `-d`, transmitir a ese receptor (consulte [Prueba con receptor par](#receiver)) |
| `--scan [seconds]` | Mostrar los dispositivos cercanos y salir (predeterminado: 10 segundos) |
| `-h` | Mostrar la ayuda |

Para desarrollo y pruebas también existen `--hci-tcp <host:port>` (un controlador virtual por TCP en lugar del adaptador WinUSB), `--test-tone` (1 kHz a la izquierda / 1.5 kHz a la derecha en lugar del audio capturado), `--duration <s>` (detenerse tras ese número de segundos) y `--hci-capture <file.pklg>` (captura HCI de toda la ejecución); consulte [Compilación](building#emu).

Sin `--cli`, `a2dpwb-core.exe` es el programa residente: también inicia la ventana, salvo con `--background` (solo el icono de la bandeja; lo usa **Iniciar con Windows** cuando **Iniciar en la bandeja** está activado). `a2dpwb-core.exe --quit` pide a un A2DPWB en ejecución que detenga la transmisión y se cierre (lo usa el instalador).

## Modos de captura {#capture-modes}

A2DPWB admite tres modos de captura de audio. Cada uno graba un punto distinto del recorrido del audio:

| Modo | Qué graba |
|:-----|:----------------|
| Loopback del sistema | Lo que reproduce el dispositivo de salida predeterminado de Windows: todas las aplicaciones mezcladas |
| Dispositivo virtual | Lo que las aplicaciones reproducen en el dispositivo seleccionado (p. ej., VB-CABLE) |
| Aplicación | Solo lo que reproduce una aplicación (y sus procesos secundarios), después del procesamiento propio de la aplicación. Requiere Windows 11 o Windows 10 versión 2004 (compilación 19041) o posterior |

**Dispositivos de audio** muestra la misma explicación en **Origen de captura**. Un dispositivo nuevo captura con Loopback del sistema, que funciona sin instalar nada más; elija allí Dispositivo virtual o Aplicación para él.

El Loopback del sistema copia el audio; no se lo quita al dispositivo de salida predeterminado. El audio sigue sonando en ese dispositivo (altavoces, un televisor HDMI, ...) además de en los auriculares Bluetooth. Para oírlo solo en los auriculares, use el modo Dispositivo virtual con un dispositivo que no reproduzca en ningún sitio, como VB-CABLE.

En el modo Dispositivo virtual, **Cambiar dispositivo predeterminado** convierte el dispositivo seleccionado en la salida predeterminada de Windows durante la transmisión, de modo que las aplicaciones reproducen en él sin cambiar la configuración de Windows. Al detener la transmisión, A2DPWB vuelve al dispositivo predeterminado anterior.

- La lista **Dispositivo de audio** muestra solo los dispositivos que A2DPWB detecta como dispositivos de audio virtuales. Si no hay ninguno, la pantalla **Dispositivos de audio** ofrece **Descargar VB-Cable**.
- El dispositivo de comunicación predeterminado (el que usan las aplicaciones de llamadas) no se cambia.
- Algunas aplicaciones siguen reproduciendo en el dispositivo anterior hasta que se reinician.
- Si A2DPWB se cierra inesperadamente, el dispositivo predeterminado no se restaura. Cámbielo en la configuración de Sonido de Windows.

### Modo Aplicación {#application-mode}

El modo Aplicación envía el audio de una aplicación, por ejemplo un reproductor de música, un navegador o una aplicación de efectos. Elija la aplicación en la lista **Aplicación**. La lista muestra las aplicaciones que están reproduciendo audio en ese momento; si la suya no aparece, inicie la reproducción en ella y haga clic en **Actualizar**. Se incluyen los procesos secundarios de la aplicación, así que también funciona un navegador que reproduce el audio desde un proceso aparte.

- No es necesario que la aplicación esté en ejecución. A2DPWB se conecta igualmente a los auriculares y empieza a enviar en cuanto la aplicación se inicia. Si la aplicación se cierra durante la transmisión, A2DPWB sigue conectado y la vuelve a captar cuando se reinicia.
- La aplicación sigue sonando también en su propio dispositivo de salida. Para oírla solo en los auriculares, silencie ese dispositivo de salida (su volumen maestro): esto no afecta a lo que captura A2DPWB. Pero no silencie la aplicación en el Mezclador de volumen de Windows, porque los auriculares también recibirían silencio.
- El control deslizante de la aplicación en el Mezclador de volumen de Windows cambia el nivel enviado a los auriculares (y el de su dispositivo de salida). El volumen maestro y las teclas de volumen no cambian el audio enviado; con **Sincronizar volumen de Windows** activado, cambian en su lugar el volumen de los auriculares (consulte [Volumen](#volume)).
- Windows convierte el audio a la frecuencia de muestreo que pide A2DPWB, así que el formato del dispositivo en la configuración de Sonido no lo limita.
- En Windows 10 anterior a la versión 2004 (compilación 19041), Windows no permite capturar por aplicación. Al conectar un dispositivo configurado con captura de Aplicación se muestra entonces un error que pide otro origen de captura.

### Volumen {#volume}

Durante la transmisión, Inicio y el panel desplegable de la bandeja muestran un control deslizante **Volumen** cuando los auriculares admiten el volumen absoluto de AVRCP. Ajusta el volumen en los propios auriculares, de modo que el audio enviado conserva toda su resolución. El control deslizante también sigue los botones de volumen y los controles táctiles de los auriculares.

**Sincronizar volumen de Windows** (en **Dispositivos de audio**, para todos los dispositivos; activado de forma predeterminada) hace que un volumen de Windows y el volumen de los auriculares sean uno solo durante la transmisión. El volumen de Windows sincronizado es el del dispositivo que captura A2DPWB: el dispositivo de salida predeterminado en los modos Loopback del sistema y Aplicación, el dispositivo seleccionado en el modo Dispositivo virtual. Desactivado, el volumen de Windows y el de los auriculares son independientes.

- Los botones y gestos de volumen de los auriculares, y el control deslizante **Volumen**, mueven el volumen de Windows sincronizado, y su control deslizante en la configuración de Sonido mueve el volumen de los auriculares.
- Las teclas de volumen y el control flotante de volumen de Windows siempre actúan sobre el dispositivo de salida predeterminado, así que solo cambian el volumen de los auriculares cuando el dispositivo de salida predeterminado es el sincronizado (siempre, salvo en el modo Dispositivo virtual sin **Cambiar dispositivo predeterminado**).
- Al iniciar la transmisión, el volumen de Windows pasa a ser el de los auriculares; al detenerla, el dispositivo recupera el volumen que tenía antes.
- El audio se captura antes de aplicar el volumen de Windows, por lo que se reduce una sola vez, en los auriculares. Si el volumen sincronizado también cambia el audio capturado, el volumen se aplica dos veces: en ese caso desactive la sincronización (consulte [Aplicaciones de efectos para todo el sistema](#effects-apps)).
- El silencio no se sincroniza: silenciar el dispositivo de salida deja el PC en silencio, pero no silencia los auriculares.

Las versiones anteriores permitían que un dispositivo siguiera el volumen de otro dispositivo de salida (en modo de desarrollador); esa opción ya no se usa.

**Recordar el último volumen del dispositivo de audio Bluetooth** (en **Dispositivos de audio**, para todos los dispositivos; activado de forma predeterminada) guarda el volumen propio de cada dispositivo de audio y lo vuelve a aplicar en cuanto se conecta, antes de enviar audio, así que el volumen de un dispositivo no pasa a otro. Desactívelo si quiere que cada dispositivo empiece con su propio último volumen, que puede ser mucho más alto que el usado la última vez con A2DPWB (p. ej., después de usarlo con un teléfono).

### Botones de los auriculares {#headphone-buttons}

Los controles de reproducir/pausar, pista siguiente y pista anterior de los auriculares (botones o gestos táctiles, p. ej. un doble toque en los WH-1000XM4) controlan la reproducción en el PC durante la transmisión: A2DPWB los pasa a Windows como teclas multimedia, que llegan a la aplicación que aparece en los controles multimedia de Windows (Spotify, un navegador, un reproductor...). Detener también se transmite.

- Reproducir y pausar actúan ambos como la tecla Reproducir/Pausar de Windows, de modo que un toque siempre alterna entre reproducción y pausa, aunque la idea que tienen los auriculares del estado no esté actualizada.
- A2DPWB indica a los auriculares si se está reproduciendo audio (según haya o no audio que capturar), para que los auriculares que se pausan al quitárselos envíen el comando correcto.
- Avance rápido / retroceso (mantener pulsado un botón) no tienen tecla multimedia en Windows y se ignoran.
- Los gestos de volumen cambian el volumen propio de los auriculares; con **Sincronizar volumen de Windows**, el volumen de Windows sincronizado los sigue (consulte [Volumen](#volume)).
- Desactívelo para un dispositivo con **Dispositivos de audio > Los botones del dispositivo de audio controlan la reproducción**, p. ej. si otro programa ya reacciona a las teclas multimedia y estas actúan dos veces.

Esto requiere AVRCP en los auriculares, que casi todos tienen. Sin él, la transmisión funciona como siempre, solo que sin los botones.

### Aplicaciones de efectos para todo el sistema {#effects-apps}

Una aplicación de efectos para todo el sistema (un ecualizador, un potenciador de sonido, una aplicación de sonido envolvente virtual, ...) suele instalar su propio dispositivo de salida virtual. Las demás aplicaciones reproducen en ese dispositivo; la aplicación de efectos procesa el audio y reproduce el resultado en el dispositivo de salida que usted elige en ella. El modo de A2DPWB que elija decide si los auriculares reciben los efectos:

| Configuración | Qué llega a los auriculares |
|:--------|:----------------------------|
| Dispositivo virtual: el dispositivo virtual propio de la aplicación de efectos | El audio **antes** de los efectos: lo que las aplicaciones envían a la aplicación de efectos |
| Aplicación: la aplicación de efectos | El audio **después** de los efectos: lo que reproduce la aplicación de efectos (recomendado) |
| Dispositivo virtual: el dispositivo de salida de la aplicación de efectos (p. ej., VB-CABLE configurado como su salida) | El audio después de los efectos (para versiones de Windows sin modo Aplicación) |

En el modo Aplicación, el dispositivo de salida de la aplicación de efectos puede ser cualquiera: A2DPWB toma lo que reproduce la aplicación, no lo que reproduce el dispositivo, así que el dispositivo solo es un destino para el audio. Para que el PC no reproduzca el audio al mismo tiempo, elija una salida que nadie escuche (p. ej., el audio HDMI de un monitor) o silencie el volumen maestro de ese dispositivo. Un dispositivo virtual como VB-CABLE también sirve como destino silencioso, pero no es necesario. El dispositivo debe estar habilitado y conectado.

Si el dispositivo virtual propio de la aplicación de efectos es el dispositivo de salida predeterminado de Windows, su volumen también reduce lo que recibe la aplicación de efectos. Con **Sincronizar volumen de Windows** activado, un cambio de volumen se aplica entonces dos veces (en la entrada de la aplicación de efectos y en los auriculares). Desactívelo y use el volumen de los auriculares o el control deslizante **Volumen**.

#### Por qué A2DPWB no tiene su propio dispositivo de salida virtual {#no-virtual-device}

Una entrada de auriculares Bluetooth en la lista de dispositivos de salida de Windows, que una aplicación de efectos pudiera seleccionar directamente, requeriría un controlador de audio en modo kernel. A2DPWB no incluye ninguno:

- Escribir y mantener un controlador de audio de kernel es un trabajo enorme, y un error en él puede bloquear Windows (pantalla azul).
- Windows 10 y 11 solo cargan controladores firmados por Microsoft. Obtener esa firma requiere un certificado de firma de código EV (varios cientos de dólares estadounidenses al año) y una cuenta de Microsoft Partner Center, normalmente registrada como empresa.
- La única forma de evitar la firma es el modo de firma de prueba de Windows, que reduce la seguridad del sistema y hace fallar algunos programas (p. ej., juegos con antitrampas). A2DPWB no va a depender de él.

El modo Aplicación, o un dispositivo virtual gratuito como VB-CABLE, cubre el mismo uso.

- Windows no tiene una API pública para cambiar el dispositivo predeterminado. A2DPWB usa la interfaz no documentada IPolicyConfig, como otras herramientas de cambio de audio (Windows 10 y 11).

{: .warning }
Loopback del sistema y Dispositivo virtual usan el modo compartido de WASAPI: con la frecuencia de muestreo en Automático (también con la selección automática), A2DPWB pide 48 kHz, o la mejor frecuencia que anuncien los auriculares si no tienen 48 kHz, y WASAPI remuestrea a ella el **Formato predeterminado** del dispositivo. Para usar LDAC a 96 kHz, elija 96 kHz en los ajustes del códec; no hace falta cambiar el formato predeterminado en **Configuración de sonido > Propiedades del dispositivo > Opciones avanzadas**.

## Comparación de códecs

| Códec | Ideal para | Inconveniente |
|:------|:---------|:----------|
| LDAC (HQ) | Máxima calidad de audio | Mayor latencia, necesita una conexión estable |
| LDAC (ABR) | Calidad + estabilidad | La tasa de bits varía con la conexión |
| aptX HD | Alta calidad, mayor compatibilidad de dispositivos | 576 kbps fijos |
| aptX | La mayor compatibilidad entre dispositivos aptX | 16 bits, 352/384 kbps |
| aptX LL | Juegos, vídeo (baja latencia) | Menor calidad de audio (16 bits) |
| AAC | Dispositivos Apple, buena eficiencia | Calidad moderada |
| SBC | Máxima compatibilidad | La menor calidad de audio |

La pantalla **Códecs** muestra la misma comparación como tres barras en cada tarjeta de códec.

## Familia aptX y aptX Adaptive {#aptx-compatibility}

A2DPWB admite tres códecs aptX codificados con [libopenaptx](https://github.com/pali/libopenaptx) y aptX Adaptive, codificado con el codificador de [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD y aptX Low Latency son experimentales.** Siguen las implementaciones de Android / PipeWire y superan las pruebas de ida y vuelta de codificación/decodificación, pero aún no se han verificado con auriculares reales.

{: .note }
**¿Solo necesita aptX clásico?** Windows 10 ya admite aptX clásico en su pila Bluetooth integrada (no aptX HD, aptX LL ni aptX Adaptive), así que A2DPWB no es necesario para ese códec. A2DPWB es útil sobre todo para LDAC, aptX HD y aptX Low Latency.

| Códec | Vendor ID / Codec ID | Encabezado RTP | Estado |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Sí | Compatible (265-427 kbps, 48 / 96 kHz, 24 bits) |
| aptX HD | 0x000000D7 / 0x0024 | Sí | Compatible (576 kbps, entrada de 24 bits) |
| aptX | 0x0000004F / 0x0001 | No | Compatible (352/384 kbps a 44.1/48 kHz) |
| aptX Low Latency | 0x0000000A o 0x000000D7 / 0x0002 | No | Compatible (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**aptX Adaptive está confirmado con auriculares reales a 48 kHz:** un usuario lo transmitió a unos EarFun Air Pro 4+ en 2.0.2-beta.6 sin cortes ni tirones, tan estable como el aptX clásico ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)). Además supera las pruebas unitarias, el conjunto de pruebas bit a bit del proyecto original y la prueba de extremo a extremo contra el receptor Bluetooth virtual (`tools/emu`, que comprueba el encabezado RTP, el tamaño de los paquetes y los tipos de trama; el audio no se decodifica, ya que no existe un decodificador abierto). El proyecto original informa de que funciona en Linux / PipeWire con Bose QuietComfort Ultra (1.ª generación) y B&W Px7 S2e a 48 kHz en Alta y Baja latencia. 96 kHz (Alta 646 kbps, Estándar 340, Móvil 255) solo se usa cuando se elige en su configuración (o con `--rate 96000`) y los auriculares son un receptor R2 que lo anuncia, nunca en Baja latencia; 96 kHz también está confirmado ya con auriculares reales ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)). Automático, también la selección automática, transmite a 48 kHz. [Se agradecen los informes](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11).

El codificador es el codificador aptX Adaptive en Rust puro de [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). El proyecto original lo describe como una reimplementación en sala limpia cuyas tablas numéricas se reconstruyeron a partir de binarios de Qualcomm distribuidos y obtenidos legalmente y de capturas por aire; no incluye código, SDK ni binarios de Qualcomm (consulte [Licencias](licenses)).

- **Cómo elegirlo**: Automático lo elige cuando los auriculares no ofrecen LHDC V5 ni LDAC. O elija **aptX Adaptive** en **Códecs**, o `-c aptxad`.
- **Formato**: 48 kHz estéreo, 24 bits; 96 kHz en calidad alta, si se elige, a un dispositivo que lo anuncie (aptX Adaptive R2); una solicitud de 44.1 kHz se captura a 48 kHz.
- **Auriculares**: receptores R1 (sin extensión de capacidades, p. ej. B&W Px7 S2e) y receptores R2 (flujo de bits R2).

| Calidad (**...** / `-q`) | Receptores R1 | Receptores R2 | Paquetes |
|:-------------------------|:---------|:---------|:--------|
| Alta (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 bytes |
| Estándar (`sq`; el valor predeterminado de la referencia) | 364 kbps | 270 kbps | 14 ms |
| Móvil (`mq`) | 318 kbps | 204 kbps | 16 ms |
| Baja latencia (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 bytes |

`ll` solo lo usa aptX Adaptive; los demás códecs lo tratan como Alta.

**ABR** (ajuste **ABR** del códec, CLI `-a`): en Alta, la tasa de bits baja un nivel (no por debajo de Móvil) mientras la cola de envío tiene 3 o más paquetes, como mucho cada ~0,5 s, y vuelve a subir un nivel (hasta la calidad elegida) tras ~3 s con como mucho un paquete en cola. Baja latencia no tiene ABR.

### No hay «recurso» de aptX Adaptive a aptX

Cuando A2DPWB se conecta, los auriculares anuncian una **lista de códecs** (extremos de transmisión AVDTP). A2DPWB elige un códec de esa lista. aptX Adaptive y aptX clásico son entradas distintas de la lista, así que dentro del códec nada «recurre» de uno al otro:

- **Los auriculares también ofrecen aptX clásico** -- Muchos auriculares aptX Adaptive lo hacen, porque Qualcomm promociona aptX Adaptive como retrocompatible con aptX / aptX HD. Automático elige aptX Adaptive (por encima de aptX HD y aptX); aptX clásico se usa cuando usted lo elige, como un códec independiente en la lista de los auriculares, no un modo reducido de aptX Adaptive.
- **Los auriculares solo ofrecen aptX Adaptive** (sin aptX clásico, aptX HD ni aptX LL) -- si elige aptX, se recurre al siguiente códec inferior que ofrezcan, normalmente AAC o SBC; elija aptX Adaptive (o Automático) para usarlo.

Prioridad de la selección automática: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream y Opus 05, ambos experimentales, solo si se eligen)

### El códec seleccionado no está disponible {#codec-fallback}

Cuando solicita un códec concreto que los auriculares no ofrecen:

| Dónde | Comportamiento |
|:------|:---------|
| **Usar otro códec compatible si este no está disponible** activado (predeterminado) | Se conecta con el mejor códec que ofrecen los auriculares, en el orden de prioridad de Automático, y la insignia del códec indica durante unos segundos qué códec se usa en su lugar y por qué. |
| Opción desactivada | La conexión se detiene con un error («El dispositivo no admite ...»). Si solicitó aptX, aptX HD o aptX LL y los auriculares solo ofrecen aptX Adaptive, el mensaje lo indica y sugiere aptX Adaptive, Automático, AAC o SBC. |
| CLI (`-c <codec>`) | Se detiene con el mismo error. Use `-c auto` para conectar con el mejor códec ofrecido. |

La opción está en la parte superior de la pantalla **Códecs**, para todos los dispositivos (no se usa con Automático).

### Falta un códec mientras los auriculares están conectados a otro dispositivo {#sep-in-use}

Los auriculares ofrecen cada códec en un extremo de transmisión (SEP). En una conexión doble (multipunto), un extremo que transmite al teléfono o al PC se indica como **en uso**, y su códec no se puede usar hasta que esa transmisión se detenga. Algunos auriculares además solo ofrecen códecs de alta resolución (LDAC, L2HC, ...) mientras hay un único dispositivo conectado. A2DPWB registra entonces:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

y el error indica que hay extremos en uso por otro dispositivo. Desconecte los auriculares del teléfono / PC (o desactive la conexión doble en la aplicación de los auriculares) y vuelva a conectar. Un PC cuyo Bluetooth integrado siga emparejado con los auriculares también cuenta como otro dispositivo.

### Frecuencia de muestreo

aptX, aptX HD y aptX LL solo admiten 44.1 kHz y 48 kHz (aptX Adaptive 48 kHz, o 96 kHz si se elige y los auriculares lo anuncian). A2DPWB elige una de las frecuencias que anuncian los auriculares (en Automático 48 kHz, si no 44.1 kHz) y deja que WASAPI remuestree la captura. No es necesario cambiar la configuración de Sonido de Windows para aptX.

### Comprobar qué ofrecen sus auriculares

1. Conéctese a los auriculares.
2. Abra **Diagnóstico** y escriba `remote` en el filtro del registro.

Las mismas líneas están en los archivos de registro de la carpeta de configuración (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log`); en modo CLI se muestran en la consola. Las líneas de capacidades muestran exactamente qué códecs ofrecen los auriculares:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- aptX clásico está disponible (aunque también aparezca `aptX Adaptive=true`); Automático lo usa.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` con `aptX Adaptive=true` -- los auriculares solo ofrecen aptX Adaptive; elija aptX Adaptive, o Automático, AAC o SBC.
- `aptX Adaptive=true [not usable]` -- los auriculares ofrecen aptX Adaptive sin 48 kHz estéreo, que A2DPWB necesita.
- `remote vendor codec ... - unsupported` -- otro códec de fabricante que A2DPWB no implementa.

## Tamaño máximo del paquete multimedia (por dispositivo) {#max-packet}

**Tamaño máx. de paquete multimedia** de cada dispositivo (pantalla **Desarrollador**, un campo por dispositivo guardado; CLI: `--max-packet <bytes>`) fija el límite superior de los datos de audio de cada paquete Bluetooth, de 679 a 1679 bytes, para que los dispositivos de audio que necesitan otro valor puedan tener el suyo. La pantalla Desarrollador se muestra en modo de desarrollador (**Configuración > Modo de desarrollador**); la mayoría de los usuarios nunca lo necesita. El límite propio del dispositivo de audio (MTU) también se aplica siempre; A2DPWB usa el menor de los dos. El cambio se aplica a partir de la siguiente conexión. Antes era un ajuste del perfil (en **Avanzado**): un dispositivo conserva el valor del perfil que se mantuvo (consulte [Códecs](#codecs)); los perfiles guardados por la v1 anterior a la 1.1 toman el valor que entonces se configuraba de forma global.

**Se recomienda 1023 (predeterminado).** Cabe en un paquete de banda base Bluetooth (3-DH5). Los valores mayores solo ahorran un 1-2 % de sobrecarga, y cada paquete perdido supone entonces perder más audio. 679 cabe en un paquete 2-DH5 y es el mínimo que acepta LDAC.

## Verificar el flujo multimedia (a2dpwb_decode) {#verify-stream}

La mayoría de los auriculares no pueden mostrar qué códec se está usando. **Diagnóstico > Iniciar captura HCI** / **Detener captura HCI** graba un registro de paquetes HCI (`hci_<date>_<time>.pklg` en la carpeta de configuración) con la negociación del códec y todos los paquetes multimedia enviados. Con LDAC a 990 kbps crece aproximadamente 0.5 GB por hora, así que capture solo el tiempo que necesite. En modo CLI, `--hci-capture <file.pklg>` graba toda la ejecución.

Puede iniciar una captura antes de conectar o durante la transmisión. Si se inicia durante una conexión, A2DPWB escribe primero los paquetes de establecimiento de la conexión que ha recordado (establecimiento del canal L2CAP y señalización AVDTP, incluida la negociación del códec), para que el archivo pueda analizarse igualmente. Diagnóstico muestra la ruta del archivo mientras graba y después de detenerla.

Abra la captura en Wireshark o compruébela con la herramienta para desarrolladores `a2dpwb_decode` (se compila desde el código fuente; consulte [Compilación](building)):

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

La herramienta sigue la señalización AVDTP (SET_CONFIGURATION / RECONFIGURE y si los auriculares la aceptaron), extrae los paquetes multimedia que envió A2DPWB y los comprueba frente a la configuración negociada:

| Códec | Qué se comprueba | Salida |
|:------|:----------------|:-------|
| SBC, AAC, aptX, aptX HD, aptX LL | Decodificación completa con los decodificadores de referencia; parámetros de trama frente a la configuración; número de tramas en el encabezado de la carga útil multimedia | `<prefix>.<n>.<codec>.wav` para escuchar |
| LDAC | Solo los encabezados de trama (palabra de sincronización, frecuencia de muestreo, modo de canal, longitud de trama), número de tramas, tasa de bits. No existe ningún decodificador LDAC de código abierto | Solo informe |

Para cada flujo también informa de los saltos en la secuencia RTP, la velocidad a la que avanza la marca de tiempo RTP, la tasa de bits y cuántos paquetes ACL indicó el controlador Bluetooth como completados. La última línea es `RESULT: OK` o `RESULT: PROBLEMS FOUND` (código de salida 0 o 1).

{: .note }
Esto muestra lo que envió A2DPWB y que es válido para el códec negociado. Como un receptor solo puede decodificar el códec negociado, un audio correcto en los auriculares junto con un informe sin errores es una prueba sólida de que el códec se está usando de verdad.

## Calidad del enlace y AFH {#link-quality}

**Diagnóstico > Calidad del enlace** muestra lo que envía A2DPWB durante la transmisión: la tasa de bits real, los paquetes enviados, los paquetes descartados antes del envío (cola de envío llena: el enlace de radio no daba abasto), el audio descartado en la captura, la cola de envío, el tiempo de radio usado y la **Señal**: el RSSI de la conexión, relativo al rango óptimo de recepción del controlador (0 = bien, negativo = demasiado débil; no es un nivel absoluto), y en cuántos de los 79 canales Bluetooth salta el enlace (AFH). Inicio y el panel desplegable de la bandeja lo resumen como Buena, Aceptable o Mala.

Bluetooth evita por sí mismo los canales ocupados (AFH), pero solo se entera de la presencia de Wi-Fi a través de los errores. A2DPWB además indica a su controlador qué canales debe evitar (clasificación de canales AFH del host), en cada conexión:

| AFH | |
|:--|:--|
| Automático (predeterminado) | Solo el canal Wi-Fi de 2.4 GHz al que está conectado este PC (se vuelve a comprobar cada 30 segundos mientras hay conexión). Nada si el PC no está en una Wi-Fi de 2.4 GHz: desconectado, en 5 o 6 GHz, o por cable |
| Desactivado | Nada: el controlador decide por su cuenta |
| Manual, p. ej. `6,11` | Estos canales Wi-Fi (1 a 14) |

Automático deja al AFH propio del controlador las redes que el PC solo oye: evitar el canal ocupado de un vecino resta 22 MHz al salto de frecuencia y puede empeorar el enlace en lugar de mejorarlo. Para evitar un canal así de todos modos, introdúzcalo en Manual.

Siempre quedan en uso al menos 20 canales Bluetooth. Configúrelo en la pantalla **Adaptador**, con `"afh"` en `settings.json` (carpeta de configuración) o con `--afh` en modo CLI. La línea de radio muestra qué canales Wi-Fi se evitan. Que sirva o no depende del entorno: mídalo con la prueba con receptor par.

## Prueba con receptor par (tools/linux_sink) {#receiver}

La vista de calidad del enlace muestra lo que envía A2DPWB. Para ver lo que llega realmente, ejecute `tools/linux_sink/a2dpwb_sink.py` en un segundo PC con Linux. Controla directamente el adaptador Bluetooth normal de ese PC (sirve el integrado) mediante Bumble sobre el canal de usuario HCI, como receptor A2DP para todos los códecs incluido LDAC, mide cada paquete multimedia que llega y ofrece las estadísticas por la red:

Copie la carpeta `tools/linux_sink` al PC con Linux (basta con la carpeta) y después:

```bash
sh setup.sh      # una vez: bibliotecas de decodificación, entorno de Python (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # iniciar el receptor (root: toma el control del adaptador Bluetooth)
```

En el PC con Windows, cierre A2DPWB (bandeja > **Salir**) y ejecute:

```bash
a2dpwb-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` encuentra el receptor en la red local, transmite a su adaptador Bluetooth salvo que `-d` indique otro dispositivo, e imprime las estadísticas del receptor cada 5 segundos y al final: tasa de bits, paquetes perdidos y tardíos (números de secuencia RTP), jitter y huecos, cortes y saltos de un búfer de reproducción modelado, errores de flujo / decodificación y el RSSI en el receptor (relativo a su rango óptimo de recepción: 0 = bien, negativo = demasiado débil). Sin `--test-tone` envía el audio del sistema. `--remote-sink <host[:port]>` indica el receptor en lugar de buscarlo.

{: .note }
Con **Configuración > Modo de desarrollador**, Diagnóstico tiene una pestaña **Prueba con receptor par**. En esta versión es una vista previa: guarda la dirección del receptor, pero todavía no transmite. Use la línea de comandos anterior.

Ambos lados cuentan los paquetes de un flujo desde su primer paquete, de modo que se pueden comparar los paquetes enviados y recibidos: deberían coincidir. Bluetooth retransmite dentro de los controladores los paquetes de radio perdidos, y A2DPWB no abandona un paquete mientras el enlace está activo, así que un enlace de radio deficiente se manifiesta como huecos, jitter y cortes en el receptor, no como paquetes perdidos; cuando el enlace no da abasto, los paquetes se descartan antes del envío en el lado emisor. Ninguno de los dos lados puede conocer el número de retransmisiones.

Mientras la herramienta se ejecuta, la pila Bluetooth del propio receptor no puede usar su adaptador. La búsqueda usa una difusión UDP en el puerto 51201, así que ambos PC deben estar en el mismo segmento de red (si no, escriba la dirección). Consulte `tools/linux_sink/README.md` para ver todas las opciones y el protocolo de estadísticas.

## LE Audio (experimental) {#le-audio}

A2DPWB también puede transmitir a auriculares LE Audio (LC3 sobre flujos isócronos conectados, BAP unicast), con el volumen del auricular (VCP), los botones multimedia (MCP), los dos auriculares de un par (CSIP) y un modo de baja latencia para auriculares gaming (GMAP). Esta función es **experimental** y se elige por dispositivo: solo se usa para la entrada LE Audio de un dispositivo. Solo se ha probado con un emulador de Bluetooth, **todavía no con auriculares LE Audio reales**, y puede que no funcione con los suyos; para el uso diario, use el A2DP clásico.

Qué se necesita:

- Un adaptador Bluetooth USB con **Bluetooth 5.2 o posterior que admita canales isócronos (ISO)**, con WinUSB asignado como para A2DP. Un adaptador sin ISO se rechaza con «adapter does not support LE Audio (no ISO buffers)».
- Auriculares compatibles con LE Audio (LC3); algunos necesitan activar antes LE Audio en su propia aplicación.

Cómo usarlo:

1. Ponga los auriculares en modo de emparejamiento y busque dispositivos en **Dispositivos de audio**. Los dispositivos LE Audio llevan la etiqueta **LE Audio**, un par de auriculares **Conjunto LE Audio** (elija cualquiera de los dos: A2DPWB encuentra el otro). Unos auriculares que también admiten A2DP aparecen dos veces, una por tipo.
2. En **Códecs**, elija la tarjeta LC3 de la entrada LE Audio. No se elige códec ni calidad (A2DPWB elige la mejor configuración LC3 que aceptan los auriculares) y solo están disponibles las capturas Loopback del sistema y Dispositivo virtual.
3. Debajo, **Baja latencia para auriculares gaming (GMAP, experimental)** está activada por defecto: con los auriculares que admiten el rol de juego de GMAP usa tramas LC3 más cortas; los demás auriculares reciben la configuración multimedia en ambos casos.

No hay ningún interruptor de LE Audio en Configuración. Si el lado LE llegara a perturbar un adaptador, `"le_audio_experimental": false` en `settings.json` (**Configuración > Carpeta de configuración**) lo desactiva por completo.

En el modo CLI, `--le-audio -d <dirección>` transmite a un dispositivo LE Audio, `--le-device <dirección>` añade otro dispositivo del grupo (el segundo auricular), `--le-scan` lista los dispositivos LE Audio cercanos, y `--le-low-latency` / `--le-preset <nombre>` eligen la configuración (`a2dpwb-core.exe --cli -h` muestra todas las opciones LE).

## Actualizaciones {#updates}

A2DPWB consulta GitHub Releases como máximo una vez al día (**Configuración > Buscar actualizaciones al iniciar**; **Incluir versiones preliminares al buscar actualizaciones** también ofrece versiones beta). Cuando hay una versión nueva, Configuración lo indica:

- **Copia instalada**: **Actualizar ahora** descarga la actualización firmada. Si hay un dispositivo de audio conectado, espera y la instala cuando se desconecta, de modo que una actualización nunca corta el audio. A2DPWB se cierra entonces, instala la nueva versión y se vuelve a iniciar por sí solo.
- **Copia portable** (o una compilación sin firma de actualizaciones): **Abrir la página de descarga** abre la página de la versión.

## Solución de problemas

### La ventana indica que el núcleo no se está ejecutando

La ventana necesita `a2dpwb-core.exe` a su lado y lo inicia por sí misma. «El núcleo de A2DPWB no se está ejecutando» significa que no se pudo iniciar: reinstale A2DPWB (o vuelva a extraer el zip portable). «El núcleo es de otra versión» significa que los dos programas proceden de versiones distintas: instale la misma versión de ambos (la última versión publicada lo consigue).

### Falla la asignación o la restauración del controlador

- «Se rechazó la solicitud de administrador»: no se cambió el controlador. Vuelva a hacer clic en el botón y acepte la solicitud.
- «Error al cambiar el controlador»: consulte **Diagnóstico > Registro** para ver los detalles.
- «El adaptador no volvió a su controlador anterior»: si el adaptador se configuró con Zadig, es posible que Windows vuelva a elegir el paquete de controlador WinUSB de Zadig. Elimine ese paquete de controlador en el Administrador de dispositivos (adaptador > Desinstalar el dispositivo, con «Intentar quitar el controlador de este dispositivo»), y después desconecte y vuelva a conectar el adaptador.

### El adaptador Bluetooth no responde {#adapter-no-answer}

Si el adaptador no devuelve nada después de abrirlo (no responde a HCI Reset), A2DPWB desiste a los 5 segundos, lo abre una vez más y, si sigue sin responder, muestra «El adaptador Bluetooth no responde». Desconecte el adaptador, vuelva a conectarlo e inténtelo de nuevo (o pulse **Reiniciar Bluetooth**). No hace falta reiniciar A2DPWB: un inicio fallido libera el adaptador y cada nuevo inicio lo vuelve a inicializar.

Para evitar que un adaptador se quede bloqueado, A2DPWB restablece sus canalizaciones USB al abrirlo y envía HCI Reset al cerrarlo.

### Problemas de emparejamiento tras actualizar

A2DPWB empareja mediante SSP (Just Works) con **General Bonding**, y las claves de enlace se guardan para que los auriculares puedan reconectarse sin volver a emparejar (la v2 lee las claves de enlace de la v1). Si emparejó los auriculares con una compilación antigua de A2DPWB y ya no se conectan, elimine el emparejamiento de A2DPWB en los auriculares (consulte el manual de los auriculares para borrar la lista de emparejamientos) y vuelva a emparejarlos.

### Tiempos de espera de conexión

A2DPWB registra registros SDP (A2DP Source, AVRCP Controller / Target) para que los auriculares lo identifiquen como fuente de audio. Si un intento de conexión agota el tiempo de espera, la conexión semiabierta se cierra, de modo que puede volver a intentarlo sin reiniciar A2DPWB. Asegúrese de que los auriculares estén encendidos, dentro del alcance y no conectados a otro dispositivo.

Una conexión fallida se reintenta automáticamente (hasta 3 intentos) cuando el enlace se estableció y luego no transmitió nada, y cuando los auriculares no respondieron. Si los auriculares ya no reconocen el emparejamiento (se restablecieron o se emparejaron con otro PC), se descarta la clave guardada y se vuelve a emparejar una vez. El error indica entonces qué ha ocurrido:

| Mensaje | Significado |
|:--|:--|
| El dispositivo no respondió | Apagado, fuera de alcance o no disponible para conexión (si es nuevo: modo de emparejamiento) |
| La conexión se estableció, pero no transmitió nada | Condiciones de radio: vuelva a intentarlo o cambie de sitio el adaptador / el dispositivo |
| El dispositivo respondió, pero luego cerró la conexión por sí mismo | Ahora no acepta conexiones: se está apagando, se está cargando o está conectado a otro dispositivo |
| Error de autenticación, también tras volver a emparejar | Ponga el dispositivo en modo de emparejamiento, o elimine su entrada de la lista de emparejamientos, y vuelva a intentarlo |

**La primera conexión** con unos auriculares necesita el modo de emparejamiento; las siguientes se reconectan con la clave de enlace guardada. Si falla aunque estén en modo de emparejamiento, puede que sigan conectados a un teléfono u otro PC (también mediante LE Audio): desconéctelos allí, desactive LE Audio en la app de los auriculares si tiene esa opción, póngalos en modo de emparejamiento y vuelva a conectar. El registro muestra entonces `signaling connection failed (0x66)` (rechazado por seguridad: los auriculares conservan la clave de un emparejamiento anterior, por ejemplo con el Bluetooth de Windows de este PC) o `(0x67)` (rechazado por falta de recursos: ocupados con otro dispositivo).

Una conexión perdida se restablece automáticamente (hasta 10 intentos). Si el PC entra en suspensión durante la transmisión, A2DPWB vuelve a conectar al reanudarse; estos intentos no se cuentan.
