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

Inicie **BlueCaster** desde el menú Inicio (o `bluecaster.exe`). El menú de la izquierda tiene las pantallas siguientes (Inicio, Códecs, Dispositivos de audio, Adaptador, Diagnóstico, Configuración, Acerca de y, en el modo de desarrollador, Desarrollador y, si se activa allí, Mezclador de volumen); se reduce a iconos cuando la ventana es estrecha. La ventana solo muestra lo que hace el programa residente `bluecaster-core.exe`: cerrarla no detiene la transmisión (consulte [Bandeja y panel desplegable](#tray)).

### Inicio

- Una tarjeta para cada dispositivo de audio guardado, una junto a otra: su estado, la batería y, en el conectado, el códec en uso (una lista en lugar de su nombre, véase más abajo) con su tasa de bits. Cuando se usa un códec distinto del elegido, la línea del códec indica el motivo junto a su nombre durante unos segundos, p. ej. **aptX Adaptive (Opus no compatible)** (o *en uso*, *es experimental*); después solo queda su marca de advertencia (al pasar el puntero se ve el motivo); la línea de estado solo indica **Conectado**
- **Conectar** / **Desconectar** bajo cada tarjeta. Solo hay un dispositivo conectado a la vez: **Conectar** en otro dispositivo desconecta el que está en uso y conecta ese
- **Volumen**: el volumen de los auriculares (consulte [Volumen](#volume)); haga clic en el altavoz para silenciar (se envía silencio) y otra vez para activar el sonido. Las tarjetas de los demás dispositivos conservan el control deslizante, desactivado
- El códec del dispositivo en uso, elegido en la lista de su tarjeta para cambiarlo rápido (por ejemplo, uno para juegos y otro para música): los códecs que el dispositivo admite, como en [Códecs](#codecs), solo por su nombre. Muestra el códec en uso, también con **Selección automática**; elegir un códec desactiva la selección automática (vuelva a activarla en Códecs). La elección se aplica al instante (el dispositivo se vuelve a conectar con él; la siguiente elección espera hasta que esté conectado, como en Códecs)
- Para el dispositivo en uso, la calidad del enlace con un pequeño gráfico, en su tarjeta mientras transmite. El origen de salida se elige en **Dispositivos de audio**; mientras una aplicación establecida como origen de salida aún no se está ejecutando, la tarjeta del dispositivo lo indica
- Un error aparece con su causa y, si lo hay, el botón que lo soluciona (**Reemplazar por el controlador WinUSB**, **Abrir página de descarga**, **Reiniciar Bluetooth**, **Reintentar**, **Códecs**, **Dispositivos de audio**, ...)
- Encima de las tarjetas, hasta que se cierran: un aviso cuando hay otro adaptador Bluetooth activo en Windows, y cuando unos auriculares OPPO activaron Hi-Res (consulte [Códecs](#codecs))

La barra lateral muestra lo mismo para cada dispositivo en todas las pantallas, y el panel desplegable de la bandeja para el dispositivo en uso.

### Códecs {#codecs}

El códec de cada dispositivo de audio guardado, una sección por dispositivo (un punto que está verde mientras el dispositivo está conectado, su nombre, etiquetas **Emparejado** / **Audio**; la dirección está en **Dispositivos de audio**) con sus tarjetas de códec, de mayor calidad de sonido primero salvo que las ponga en su propio orden, cada una con tres barras (calidad de sonido, baja latencia, estabilidad) y con la indicación de si el dispositivo lo ofreció en su última conexión: **Compatible** (marca verde) o **No compatible** (marca roja). Con **Mostrar solo los códecs compatibles** activado (el valor predeterminado), las tarjetas mostradas no necesitan la etiqueta **Compatible** y no la llevan. Los auriculares Huawei y Honor conservan su tarjeta **LDAC**, como **Desconocido**, hasta que lo anuncian: puede que solo lo hagan una vez elegido. A los auriculares OPPO que anuncian una tasa de bits máxima para LHDC V5 se les pide Hi-Res una vez cuando LHDC V5 transmite, elegido o por la selección automática (no con la calidad **Baja** ni con ABR): se vuelven a conectar una vez para ello, e Inicio lo indica cuando eso elevó su tasa de bits; después, Hi-Res sigue activado en los auriculares (a los que no les aportó nada se les devuelve al estado anterior). En el modo de desarrollador, **Volver a pedir Hi-Res**, en los ajustes de LHDC V5, olvida lo que anunció el dispositivo y que ya se le pidió, así que su siguiente conexión vuelve a pedirlo. Se ofrecen todos los códecs; los experimentales (🧪 en la tabla de códecs del README) solo se marcan en la documentación.

- **Seleccionar automáticamente el códec compatible de mayor calidad** (un ajuste para todos los dispositivos, arriba en la página; activado de forma predeterminada): cada dispositivo usa el códec de mayor calidad de sonido que admite, en este orden: LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC, con sus ajustes predeterminados: alta calidad, 24 bits si el códec los admite y 48 kHz (otra frecuencia que anuncie el dispositivo si no tiene 48 kHz, primero 44.1 kHz; la frecuencia de Windows se remuestrea), como **Auto** en los ajustes de un códec. Las tarjetas son de solo lectura, en el orden predeterminado, y el códec en uso (o el que elegirá la selección automática) aparece resaltado; al hacer clic en una tarjeta o en su **...** se pregunta si desactivar la selección automática (**Desactivar la selección automática** / **Cancelar**); una vez desactivada, se elige la tarjeta en la que hizo clic (su **...** abre sus ajustes). Desactívelo para elegir usted mismo el códec de cada dispositivo y sus ajustes; un dispositivo sin elección conserva el de la selección automática. El códec elegido para un dispositivo se conserva mientras está activado. Si la configuración anterior a 2.0.2 elegía un códec para algún dispositivo, empieza desactivado
- Con el ajuste desactivado, haga clic en una tarjeta para elegir el códec de ese dispositivo; se guarda al instante, y un dispositivo que está transmitiendo vuelve a conectarse con él. Una tarjeta que el dispositivo no admite (visible con **Mostrar solo los códecs compatibles** desactivado) también se elige: BlueCaster pide ese códec al dispositivo de todos modos y, si el dispositivo lo rechaza, la conexión se detiene con un error que lo indica (elija otra tarjeta o active la selección automática). Un códec elegido mientras el dispositivo lo ofrecía y que más tarde deja de ofrecer (auriculares que cambiaron a otro códec) pasa a la primera tarjeta del orden del dispositivo que este admita: el mejor códec que admite, salvo que haya cambiado el orden (consulte [El códec seleccionado no está disponible](#codec-fallback))
- Con el ajuste desactivado, arrastre las tarjetas de un dispositivo (la mano aparece mientras se mantiene pulsado el botón; o pulse Alt con una tecla de flecha sobre una tarjeta) para poner sus códecs en su propio orden; cada dispositivo conserva su propio orden. Un dispositivo sin códec elegido usa la primera tarjeta que admite, igual que un códec elegido que no admite. FastStream y Opus 05 (la tarjeta **Opus (PipeWire)**) nunca se usan salvo que se elijan, estén donde estén. **Restablecer el orden de los códecs** lo restablece; solo aparece mientras las tarjetas mostradas no están en su orden predeterminado (si se vuelven a colocar, vuelven a estar en el orden predeterminado). Con la selección automática no se usa el orden. Un dispositivo que transmite con un códec que esto cambia vuelve a conectarse
- Elegir otro códec para el dispositivo que está transmitiendo lo vuelve a conectar con ese códec. Después, las tarjetas (y la lista de códecs de Inicio y del panel desplegable de la bandeja) esperan hasta que vuelva a estar conectado antes de la siguiente elección, para que una serie rápida de clics no interrumpa una conexión que aún se está estableciendo. Cambiar de códec no muestra ninguna notificación de conexión; conectar un dispositivo sí
- **Mostrar solo los códecs compatibles** (activado de forma predeterminada; desactívelo para mostrar los códecs compatibles y los no compatibles): se omiten los códecs que un dispositivo no ofreció. Antes de la primera conexión de un dispositivo se muestran todos
- **...** en una tarjeta abre los ajustes de ese códec. Un cambio se aplica al instante: para el códec en uso se guarda y el dispositivo vuelve a conectarse con él, salvo el **Bitpool**, que cambia mientras escucha; para otro códec se conserva para cuando se elija ese códec. Cada códec conserva sus propios ajustes: si elige otro códec y vuelve, se recuperan. Solo se ofrece lo que ese códec admite y lo que el dispositivo anunció en su última conexión. Arriba, **Mostrar solo opciones que admite *dispositivo*** (activado de forma predeterminada; aparece cuando el dispositivo las ha anunciado), desactivado, ofrece todo lo que tiene el códec, como una frecuencia de muestreo, profundidad de bits, calidad o bitpool que el dispositivo no anunció (marcados *(no compatible)*). El dispositivo puede rechazarlos: la conexión se detiene entonces con un error que lo indica. Al volver a activarlo, lo que el dispositivo no anunció vuelve a Auto. Cada códec tiene el suyo; la selección automática nunca los usa:
  - **Calidad**: LDAC **Alta** / **Media** / **Baja** (990 / 660 / 330 kbps), LHDC V5 **Alta** / **Media-alta** / **Media** / **Baja** (1000 / 900 / 500 / 400 kbps, los cuatro niveles de Android); aptX Adaptive **Alta** / **Baja latencia** (consulte [aptX Adaptive](#aptx-adaptive)); con el ABR de LHDC V5 activado, la calidad aparece atenuada (ABR se mantiene en 400 kbps o menos). AAC, Opus, Opus 05, SBC y SBC XQ tienen uno, **Alta**, que se muestra en lugar de elegirse: la tasa de bits más alta que acepta el dispositivo (ABR o VBR, donde se ofrecen, la siguen variando; SBC y SBC XQ muestran la tasa de bits que resulta de su bitpool y de la configuración de tramas). aptX, aptX HD, aptX LL y FastStream tienen una tasa de bits fijada por el códec. Una calidad guardada por una versión anterior que el códec ya no tiene (Estándar / Móvil de aptX Adaptive, los niveles inferiores de AAC, ...) se carga como Alta
  - **Tasa de bits adaptativa (ABR)**: para LDAC, LHDC V5, aptX Adaptive, SBC y SBC XQ (consulte [ABR](#abr)); **VBR** para AAC
  - **Frecuencia de muestreo**: Auto, que se muestra con la frecuencia que da (p. ej., *Auto (48000 Hz)*), es 48 kHz, u otra frecuencia que anuncie el dispositivo si no tiene 48 kHz, primero 44.1 kHz (96 kHz añade latencia y problemas de compatibilidad sin una mejora audible, y Windows mezcla a 48 kHz); elija 96 kHz aquí para usarlo. Solo se ofrecen las frecuencias que anuncia el dispositivo (LHDC V5 no tiene 88.2 kHz; los 96 kHz de aptX Adaptive, solo a un dispositivo que los anuncie; consulte [aptX Adaptive](#aptx-adaptive)); un dispositivo que anuncia una sola frecuencia la muestra, sin Auto
  - **Profundidad de bits**: 16 o 24 bits para LDAC y LHDC V5 (Automático: 24 bits; para LHDC V5, solo si el dispositivo los anuncia), en la medida en que el dispositivo los anuncie; 24 bits para aptX HD y aptX Adaptive, 16 bits para los demás
  - **Bitpool** (SBC, SBC XQ): un control deslizante con dos tiradores, el bitpool mínimo y el máximo, dentro del rango que anuncia el dispositivo. El máximo es el bitpool que se usa; el mínimo es el más bajo que se usa (el extremo inferior del rango con el que se establece la conexión, y hasta dónde puede bajar ABR). **Automático** restablece ambos: el máximo según la calidad, el mínimo el propio del dispositivo y de ABR. La línea de debajo muestra el modo de canal, fijo para cada códec: joint stereo para SBC, canal dual para SBC XQ.
  - **Estructura de las tramas** (SBC y SBC XQ, solo en el modo de desarrollador): bloques, subbandas, asignación de bits y, para SBC, el modo de canal. Automático mantiene 16 bloques, 8 subbandas, Loudness y Joint stereo. Un valor que el dispositivo no acepta vuelve a Automático al conectar. Con un dispositivo Sony que probó un usuario, 12 bloques distorsionan el sonido y pueden desconectar (la salida del codificador se decodifica correctamente; los demás números de bloques se reproducen bien). Las elecciones siguen guardadas, pero no se usan una vez desactivado el modo de desarrollador. Las tasas de bits de AAC y LHDC V5 se mantienen dentro del máximo que anuncia el dispositivo

Los valores predeterminados dan el mejor sonido; la mayoría de los dispositivos no necesitan más que **Automático**. Un dispositivo LE Audio (experimental) tiene una tarjeta, LC3, y **Baja latencia para dispositivos de audio gaming (experimental)** (consulte [LE Audio](#le-audio)).

Por debajo, BlueCaster guarda una entrada por dispositivo en `profiles.json` (su códec, los ajustes de cada códec y su orden de códecs), llamada allí «*dispositivo* | *códec*»; la ventana y el menú de la bandeja muestran el nombre del dispositivo. Las versiones anteriores permitían crear varios perfiles para un mismo dispositivo: en el primer inicio de una versión con la pantalla Códecs se conserva el último usado (si no, el primero) y los demás se integran en él, manteniendo sus ajustes de códec (calidad, frecuencia de muestreo, profundidad de bits, ABR, configuración SBC) para sus códecs. No se conservan sus nombres, orígenes de salida ni tamaños de paquete; el `profiles.json` tal como estaba se guarda una vez como `profiles-before-codecs.json` en la carpeta de configuración. El **Orden de la selección automática…** de las versiones preliminares de 2.0.2 (un orden para todos los dispositivos, en el modo de desarrollador) ya no se usa: ahora cada dispositivo tiene su propio orden.

### Dispositivos de audio

Los auriculares y altavoces a los que BlueCaster puede transmitir, cada uno con sus propios ajustes, y debajo de ellos **Añadir un dispositivo de audio**: **Buscar dispositivos** busca dispositivos cercanos con el adaptador USB (30 segundos; aparecen a medida que responden; **Detener búsqueda** la termina; ponga los auriculares en modo de emparejamiento; mientras hay un dispositivo conectado, pide desconectarlo antes). **Añadir** guarda un dispositivo encontrado y lo conecta. Los dispositivos con los que BlueCaster ya se ha emparejado llevan la etiqueta **Emparejado**. Si la búsqueda no los encuentra, escriba la dirección en **Manual:** y haga clic en **Añadir**; las direcciones escritas o pegadas se formatean mientras escribe (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` y `AA BB CC DD EE FF` se convierten todas en `AA:BB:CC:DD:EE:FF`). Los auriculares ven el adaptador con el nombre de tu equipo seguido de BlueCaster, p. ej. *Taro-PC (BlueCaster)*, para distinguirlo del Bluetooth propio de Windows. Si el Bluetooth propio de Windows también tiene emparejado el dispositivo, se pregunta antes (**Emparejado en Windows**: quítelo allí, o **Añadir de todos modos**). Un dispositivo guardado se puede renombrar o eliminar (al eliminarlo también se eliminan sus ajustes y su emparejamiento, así que no puede volver a conectarse hasta que se empareje de nuevo), y tiene:

- **Origen de salida**: Loopback del sistema (el predeterminado de un dispositivo nuevo), Dispositivo virtual o Aplicación (consulte [Modos de captura](#capture-modes)). A su lado, el formato en que Windows mezcla el dispositivo capturado (frecuencia de muestreo y profundidad de bits, de solo lectura), en ámbar cuando el dispositivo transmite con otro (o, sin conexión, sus ajustes de códec lo piden), con una línea debajo que indica ambos formatos (sigue transmitiendo, convertido); debajo, lo que hace el origen
- **Usar como salida predeterminada de Windows mientras hay conexión**, con un dispositivo virtual

**Todos los dispositivos de audio**, encima de la lista, tiene los interruptores que se aplican a todos los dispositivos: **Sincronizar volumen de Windows** (consulte [Volumen](#volume)), **Controlar la reproducción con los botones del dispositivo de audio** y **Reconectar automáticamente**. El **Tamaño máx. de paquete multimedia** de cada dispositivo está en la pantalla **Desarrollador** (consulte [Tamaño máximo del paquete multimedia](#max-packet)).

### Diagnóstico

- **Calidad del enlace** durante la transmisión: tasa de bits real, paquetes, paquetes descartados antes del envío, audio descartado en la captura, cola de envío, el retardo que informa el dispositivo (consulte [Latencia](#latency)), tiempo de radio usado, señal (RSSI) y los canales AFH (consulte [Calidad del enlace y AFH](#link-quality))
- **Radio**: los 79 canales Bluetooth de la banda de 2,4 GHz, cuáles usa el enlace y cuáles evita el adaptador, con los canales Wi-Fi debajo, y algunos consejos. En modo desarrollador, **Escanear** añade las redes Wi-Fi de 2,4 GHz cercanas (ver [La pestaña Radio](#radio-tab))
- **Detalles del códec** durante la transmisión: la carga útil multimedia, los encabezados, la trama del códec y las tramas por paquete, y lo que anuncia el endpoint del dispositivo junto a lo que se usa: en SBC el bitpool, las frecuencias de muestreo, los modos de canal, los bloques, las subbandas y la asignación de bits; en AAC los tipos de objeto, las frecuencias de muestreo, los canales, la tasa de bits máxima y VBR; en los demás códecs las frecuencias de muestreo, las profundidades de bits y la tasa de bits máxima
- **Flujo de conexión**: las once etapas del último intento de conexión (inicio, adaptador Bluetooth, conexión ACL / AVDTP, detección de capacidades, selección de códec, captura de audio, configuración de la transmisión, codificador, inicio de la transmisión, envío de audio, mantenimiento de la conexión con desconexiones / reconexiones) como OK, Advertencia o Error, con el tiempo que tardó cada una. **Copiar flujo** lo copia
- **Registro**: en directo, filtrado por nivel (todos, info y superiores, advertencias y errores, solo errores), por texto y por el intento de conexión actual (**Solo el intento actual**). **Copiar registro visible** copia lo que se muestra. **Abrir la carpeta de registros** abre la carpeta de los archivos de registro; a diferencia de **Copiar diagnóstico**, esos archivos no están enmascarados (incluyen direcciones Bluetooth y su nombre de usuario de Windows)
- **Iniciar captura HCI** / **Detener captura HCI** (consulte [Verificar el flujo multimedia](#verify-stream))
- **Copiar diagnóstico**: la versión, el sistema operativo, el adaptador, el dispositivo de audio (códec, sus ajustes y su orden), el flujo, las advertencias / errores y el registro reciente al portapapeles, listo para pegar en un informe de errores. **Ocultar direcciones Bluetooth** (activado de forma predeterminada) sustituye las direcciones por `<addr-1>`, `<addr-2>`... (la del adaptador por `<adapter>`) y los nombres de dispositivo por `<device-1>`, `<device-2>`..., y oculta los números de serie USB, las rutas de archivo por encima de la carpeta propia de la aplicación (`<path>\BlueCaster\logs\...`) y su cuenta de Windows (nombre de usuario y SID). El mismo enmascaramiento se aplica al informe de bloqueo y a los textos de error copiados para un informe de errores
- **Informar de un problema**:
  - **Registro detallado**: **Detallado (debug)** o **Todo (trace, 1 hora)** registra mucho más sobre Bluetooth, USB, la captura de audio y la codificación. Debug sigue activado durante 24 horas, también tras reiniciar (para problemas al iniciar), y después se desactiva solo. Actívelo, reproduzca el problema y guarde después un paquete de diagnóstico
  - **Paquete de diagnóstico > Guardar…**: elija si se incluyen las capturas HCI (desactivado salvo que se las hayan pedido: conservan las direcciones y los nombres de dispositivo) y si se enmascaran las direcciones Bluetooth en los archivos de texto, y guarde un único `.zip` (en `logs\bundles\` de la carpeta de configuración; se conservan los 3 más recientes): la información de diagnóstico, los archivos de registro del núcleo, de la ventana y del asistente de controladores, los informes de bloqueo, información del sistema (adaptadores Bluetooth USB y sus controladores, dispositivos emparejados en Windows, dispositivos de audio), la configuración y los ajustes de códec de cada dispositivo (nunca las claves de enlace), y el registrador de vuelo HCI. Las capturas HCI conservan las direcciones (no las claves de emparejamiento). Se mantiene por debajo del límite de 25 MB de los adjuntos de GitHub cuando es posible (primero los registros más recientes, hasta 48 MB de texto, las 5 grabaciones y los 2 minivolcados más recientes)
  - **Informar en GitHub** (después de guardar): abre un nuevo informe de errores con la versión, la compilación de Windows, el adaptador, los auriculares, el códec, el modo de captura y la información de diagnóstico (con las direcciones enmascaradas) ya rellenados, y muestra el paquete en el Explorador para arrastrarlo a él

BlueCaster también registra algunas cosas en todo momento, para que un problema que ocurrió una sola vez se pueda estudiar igualmente:

- **Registrador de vuelo HCI**: los últimos minutos de tráfico Bluetooth se guardan en memoria (las cargas útiles multimedia recortadas, sin las claves de emparejamiento) y se escriben en `logs\hci\` cada vez que falla una transmisión o se pierde una conexión (se conservan los 10 archivos más recientes)
- **Informes de bloqueo**: si BlueCaster se bloquea, un minivolcado `.dmp` y un archivo `.txt` se guardan en `logs\crash\` (los 5 más recientes por programa)

El uso del disco está acotado: el registro de un día normal ocupa bastante menos de 1 MB y se conservan 7 días; con el registro detallado, el archivo de un día ocupa como mucho 100 MB de líneas debug y trace (después solo continúan las de info y superiores hasta el día siguiente); los archivos del registrador de vuelo ocupan unos pocos MB como mucho, y se conservan 10.

### Desarrollador {#developer}

Se muestra mientras **Configuración > Modo de desarrollador** está activado; la mayoría de los usuarios nunca lo necesita. Sirve para probar dispositivos y resolver problemas: algunos de sus ajustes solo funcionan con algunos dispositivos. Seis pestañas:

- **Ajustes de la app**: **Usar el mezclador de volumen** (desactivado de forma predeterminada): la pantalla **Mezclador de volumen**, la lista de aplicaciones del panel desplegable y el silencio de la salida (consulte [Modos de captura](#capture-modes)). No se recomienda; VB-CABLE es la mejor forma de mantener el PC en silencio
- **Ajustes de códec**: interruptores para averiguar por qué un dispositivo no suena (bit marcador RTP, registrar los primeros paquetes multimedia, configuración de AAC y opciones del codificador), el **Tamaño máx. de paquete multimedia** de cada dispositivo guardado (consulte [Tamaño máximo del paquete multimedia](#max-packet)). **Usar los ajustes de cada códec al elegir el códec automáticamente**: con la selección automática activada, el **…** de una tarjeta de códec se abre sin desactivarla, y el códec que elige la selección automática se conecta con esos ajustes en lugar de los suyos. **Igualar la frecuencia y la profundidad de bits a la salida del PC**: una frecuencia de muestreo o profundidad de bits en Automático sigue el formato del dispositivo de salida de la configuración de sonido de Windows (en la medida en que el códec y el dispositivo lo acepten) en lugar de 48 kHz. **Ignorar la tasa de bits máxima del dispositivo**: cada códec funciona a la tasa de bits de la calidad elegida aunque el dispositivo indique un máximo inferior (la tasa de bits de LHDC V5, AAC u Opus 05, el bitpool de SBC), y la pantalla **Códecs** ofrece todas las calidades y bitpools. Por encima de su máximo, un dispositivo puede cortarse o quedarse en silencio. **Límite de la cola de envío**: cuánto tiempo puede esperar un paquete multimedia en la cola de envío antes de descartarse, de 50 a 2000 ms (vacío = 350 ms); no reduce el retardo con un buen enlace (consulte [Latencia](#latency)). Todo está desactivado de forma predeterminada y se aplica al iniciarse la siguiente transmisión; mientras alguno está activado, un aviso lo indica. **Desactivar todo** los restablece
- **Fabricantes**: cuando BlueCaster usa los protocolos propios de los auriculares (Huawei: LDAC, OPPO: Hi-Res; consulte [Códecs](#codecs)), cada uno con **Protocolo propio**: **Automático** (auriculares cuyo nombre indica que son de ese fabricante; el predeterminado), **Siempre** (cualquier dispositivo) o **Nunca**. OPPO también tiene **Pedir Hi-Res en cada conexión (se reconecta cada vez)**. Los cambios se aplican al iniciarse la siguiente transmisión; mientras alguno está cambiado, un aviso lo indica, y **Desactivar todo** los restablece. No se usan una vez desactivado el modo de desarrollador
- **Registro**: cuánto tiempo se conservan los archivos de registro (7 días de forma predeterminada), el límite de tamaño diario (100 MB de forma predeterminada), un registro detallado que se mantiene activado (sin el límite de 24 horas o de 1 hora, también desde el inicio del núcleo), la línea de estado de la transmisión cada 10 segundos y **Añadir marcador** (una línea numerada en el registro, para volver a encontrar un momento). Al desactivar el modo de desarrollador se vuelve a los valores predeterminados
- **Prueba con receptor par**: transmite al receptor de medición de otro PC y compara lo enviado con lo que llegó (consulte [Prueba con receptor par](#receiver))
- **Chip**: ejecuta un script de chip en un adaptador con WinUSB: solicitudes al chip, escritas o abiertas desde un archivo, para quien añada compatibilidad con un chip (consulte [Añadir compatibilidad con chips](chip-support#chip-scripts)). Las líneas que pueden cambiar el chip solo se ejecutan después de **Permitir ejecutarlas**; Bluetooth se detiene para ello y vuelve a iniciarse en el siguiente uso

### Adaptador

- Los adaptadores Bluetooth USB detectados con su chip, dirección, ID USB, versión de Bluetooth, **Controlador** y **Firmware**; la lista se actualiza sola al conectar o desconectar un adaptador. El controlador es **Controlador original** (el que instaló Windows o el fabricante; al pasar el puntero se ve su nombre) o **Controlador WinUSB** (el que usa BlueCaster). Un adaptador Realtek muestra su dirección, tomada de su número de serie USB, incluso antes de usarse; en los demás, la dirección y la versión de Bluetooth aparecen una vez que el Bluetooth de Windows o BlueCaster ha usado el adaptador (al pasar el puntero sobre la línea se ve el motivo). Un adaptador que BlueCaster no puede usar (MediaTek, Qualcomm) conserva su controlador: sus botones de WinUSB y de firmware están desactivados. Los adaptadores Intel y Broadcom llevan la etiqueta **Experimental**. En el modo de desarrollador se le puede asignar WinUSB de todos modos, y **Leer datos del chip** en un adaptador con WinUSB guarda en `logs\probe\` lo que su chip responde a unos comandos de lectura, para quien quiera añadirle compatibilidad (Bluetooth se detiene para ello y vuelve a iniciarse en el siguiente uso); después, **Informar en GitHub** abre una incidencia con esos datos ya rellenados. Mientras se inicia un adaptador cuyo firmware carga el propio BlueCaster (Intel), la pantalla muestra la carga y si falló
- **Reemplazar por el controlador WinUSB** / **Restaurar el controlador original** (consulte [Instalación](setup#winusb)); Windows solicita permiso de administrador cada vez. Antes de restaurar, el diálogo nombra los dispositivos guardados que Windows también tiene emparejados: el emparejamiento de BlueCaster les dio una clave nueva, así que puede que haya que volver a emparejarlos en Windows
- **Chip Bluetooth** para el firmware, con **Descargar firmware**, **Abrir página de descarga** y **Abrir carpeta de firmware** (consulte [Firmware](setup#firmware))
- **Evitar canales Wi-Fi**: Automático, Desactivado o Manual, con los canales Wi-Fi al lado: los que Automático evita ahora y ninguno con Desactivado (ambos de solo lectura), los que escriba con Manual (consulte [Calidad del enlace y AFH](#link-quality))
- **Pila Bluetooth**: detenida (se inicia con la primera búsqueda o conexión), iniciando, lista o error, y **Reiniciar Bluetooth**

### Configuración

| Sección | Ajustes |
|:--------|:---------|
| Apariencia | Idioma, tema (Sistema / Claro / Oscuro) |
| Inicio | **Iniciar con Windows** e **Iniciar en la bandeja** (consulte [Iniciar con Windows](#autostart)), **Minimizar en la bandeja del sistema** y **Abrir la ventana de la bandeja desde** (consulte [Bandeja y panel desplegable](#tray)) |
| Notificaciones | **Notificar cuando se conecta un dispositivo** (el dispositivo, su batería, el códec y con qué funciona; activado de forma predeterminada), **Avisar cuando la batería esté baja**: **Por debajo del 20%** (el predeterminado), **Por debajo del 10%**, **Por debajo del 5%** o **Nunca**, una vez por conexión (de nuevo solo después de que la batería haya vuelto a estar 5 puntos por encima) |
| Actualizaciones | **Buscar actualizaciones**, **Buscar actualizaciones al iniciar**, **Incluir versiones preliminares al buscar actualizaciones** (también ofrece versiones beta), **Actualizar ahora** (consulte [Actualizaciones](#updates)) |
| Avanzado | **Modo de desarrollador** (añade la pantalla [Desarrollador](#developer), la **Estructura de las tramas** de SBC en los ajustes del códec, la etiqueta **Servicios** en **Dispositivos de audio** (consulte [Servicios y claves de emparejamiento](#shared-pairing)), **Leer datos del chip** en **Adaptador**, y más detalles al registro; no requiere reiniciar), **Carpeta de configuración** con **Abrir la carpeta de configuración**, **Ejecutar la configuración inicial** |

**Salir de BlueCaster** está al final. La versión, las licencias y **Licencias de terceros** están en la pantalla **Acerca de**.

### Configuración inicial

El primer inicio (aún sin dispositivo de audio, o sin ningún adaptador con WinUSB) abre el **Asistente de configuración**: elegir el adaptador, controlador y firmware, encontrar los dispositivos de audio (Conectar guarda el dispositivo con los mejores ajustes para él y se conecta; **Importar emparejamientos de Windows** también aparece aquí, consulte [Servicios y claves de emparejamiento](#shared-pairing)) y, después, **Configuración general**: tema, **Iniciar con Windows** e **Iniciar en la bandeja** (una primera configuración activa ambos), notificaciones y **Buscar actualizaciones al iniciar**. Consulte [Instalación](setup#first-run). **Configuración > Ejecutar la configuración inicial** la vuelve a abrir.

## Bandeja y panel desplegable {#tray}

BlueCaster sigue ejecutándose en la bandeja del sistema, con o sin la ventana:

- **Clic izquierdo** en el icono de la bandeja abre el panel desplegable: el dispositivo y su estado, **Conectar** / **Desconectar**, el volumen, el selector de dispositivos, el códec (una lista para elegirlo, como en Inicio), la calidad del enlace y **Abrir la aplicación**. Su altura se ajusta exactamente a lo que muestra. Esc o un clic en otro sitio lo ocultan. Si un error requiere una pantalla de la ventana principal (Códecs, Dispositivos de audio, el adaptador), su botón abre la ventana en esa pantalla
- **Configuración > Abrir la ventana de la bandeja desde** (**Icono de la bandeja** de forma predeterminada): en Windows 11, con **Configuración rápida (botón de volumen)**, el icono de la bandeja abre la ventana principal y el panel desplegable se abre en su lugar con la Configuración rápida, encima de sus paneles, y se cierra con ellos. Funciona allí sin cerrarlos. Si la pantalla no es lo bastante alta, se abre a su izquierda. Con **Ningún sitio** no se abre ningún panel desplegable y el icono de la bandeja abre la ventana principal.
- **Clic derecho** abre el menú: **Abrir BlueCaster**, los 10 primeros dispositivos por su nombre (una entrada LE Audio marcada con «(LE Audio)»; el que transmite, marcado; haga clic en uno para conectarlo), **Desconectar** y **Salir**. El menú se abre al instante, sin iniciar la ventana
- La información sobre herramientas indica si BlueCaster está inactivo o conectado

Con **Configuración > Minimizar en la bandeja del sistema** activado (el valor predeterminado), minimizar o cerrar la ventana solo la oculta en la bandeja, y la transmisión continúa. Si está desactivado, la ventana se minimiza en la barra de tareas y cerrarla sale de BlueCaster. **Salir** en el menú de la bandeja o **Configuración > Salir de BlueCaster** detiene la transmisión, libera el adaptador y cierra BlueCaster por completo.

## Iniciar con Windows {#autostart}

**Configuración > Iniciar con Windows** inicia BlueCaster al iniciar sesión. Con **Iniciar en la bandeja** activado, se inicia en segundo plano: solo aparece el icono de la bandeja, y la ventana se abre desde él. Si está desactivado (el valor predeterminado), se abre la ventana principal. Es el valor `BlueCaster` en `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, con los datos `"<folder>\bluecaster-core.exe"`, seguidos de `--background` cuando **Iniciar en la bandeja** está activado. Para desactivarlo a mano, elimine ese valor. Al instalar la v2 sobre la v1, **Iniciar en la bandeja** queda activado, ya que la v1 siempre se iniciaba en la bandeja.

## Modo CLI

`bluecaster-core.exe --cli` transmite desde una consola, con el mismo núcleo que la ventana. Cierre antes BlueCaster (bandeja > **Salir**): solo un programa a la vez puede usar el adaptador. La CLI no lee los ajustes de los dispositivos guardados; todo se indica como opciones.

### Básico

```bash
# Seleccionar automáticamente el mejor códec
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C detiene la transmisión. La CLI muestra el estado, el códec y la tasa de bits y, al final, el número de paquetes enviados y descartados.

### Selección de códec

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (experimental; 48 kHz, 256 kbps)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # Opus 05 de PipeWire (experimental; hacia receptores PipeWire)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz; --rate 96000 para 96 kHz)
```

Prioridad de la selección automática: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream y Opus 05, ambos experimentales, solo si se eligen)

Si los auriculares no ofrecen el códec solicitado, la CLI se detiene con un error que explica el motivo; use `-c auto` (el valor predeterminado) para tomar el mejor códec que ofrezcan, o `--fallback` para tomar el mejor códec que ofrezcan en lugar del solicitado, como hace la aplicación; consulte [Familia aptX y aptX Adaptive](#aptx-compatibility). `--no-experimental` no usa nunca los códecs experimentales (aptX HD, aptX LL, Opus, Opus 05, FastStream); la aplicación siempre los usa.

### Calidad

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # Alta, 990 kbps (predeterminado)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # Media, 660 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # Baja, 330 kbps

# LHDC V5: hq 1000 / mhq 900 / sq 500 / mq 400 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # Media, 500 kbps

# aptX Adaptive: hq 427 / ll 265 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Baja latencia
```

Los demás códecs no tienen elección de calidad: funcionan como `hq`, la tasa de bits más alta que aceptan los auriculares (aptX, aptX HD, aptX LL y FastStream: fijada por el códec). Un nivel que un códec no tiene (`mhq` en cualquier códec salvo LHDC V5, `sq` / `mq` en aptX Adaptive, `ll` en cualquier códec salvo aptX Adaptive) funciona como `hq`.

### ABR (tasa de bits adaptativa) {#abr}

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

ABR reduce automáticamente la tasa de bits cuando la conexión Bluetooth es inestable y la vuelve a subir cuando las condiciones mejoran. LDAC cambia entre sus modos de calidad; LHDC V5 sigue la política de Android y se mantiene entre 160 y 400 kbps; aptX Adaptive pasa de un nivel a otro en Alta (consulte [aptX Adaptive](#aptx-adaptive)). En la ventana, ABR está en los ajustes de un códec (**...** en su tarjeta en **Códecs**).

### Detección de dispositivos

```bash
# Dispositivos cercanos, encontrados con el adaptador USB (10 segundos; --scan 20 para más tiempo)
bluecaster-core.exe --cli --scan
```

Ponga antes los auriculares en modo de emparejamiento.

### Todas las opciones {#all-options}

| Opción | Descripción |
|:-------|:------------|
| `-d <addr>` | Dirección Bluetooth de los auriculares / del receptor (`AA:BB:CC:DD:EE:FF`; predeterminado: el primer dispositivo de audio emparejado con Windows) |
| `--device-name <name>` | Su nombre, con `-d`, como el de un dispositivo guardado (los auriculares Huawei se reconocen por él) |
| `-l` | Mostrar los dispositivos Bluetooth emparejados con Windows y salir |
| `-u <adapter>` | El adaptador USB que se usa cuando varios tienen WinUSB: ID de instancia, ruta del dispositivo o `VID:PID` (p. ej. `0bda:8771`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aptxad`, `aac`, `sbcxq`, `sbc`, `lhdcv5`, `opus`, `opus05`, `faststream`, `auto` (predeterminado: `auto`) |
| `-q <mode>` | Calidad: `hq` (Alta, predeterminado); LDAC y LHDC V5 también `sq` (Media) y `mq` (Baja), LHDC V5 también `mhq` (Media-alta); aptX Adaptive también `ll` (Baja latencia). Los demás códecs funcionan con cualquier nivel como `hq` |
| `-a` | ABR (LDAC, LHDC V5 y aptX Adaptive; SBC / SBC XQ: bitpool adaptativo) |
| `--fallback` | Un códec `-c` que los auriculares no ofrecen recurre al mejor que ofrezcan, como hace la aplicación, en lugar de dar un error |
| `--no-experimental` | No usar nunca los códecs experimentales (aptX HD, aptX LL, Opus, Opus 05, FastStream) |
| `-m <mode>` | Modo de captura: `loopback` (predeterminado), `virtual`, `app` |
| `--audio-device <id>` | Dispositivo de reproducción para `-m virtual` |
| `--app <exe>` | Ejecutable para `-m app` (p. ej. `spotify.exe`) |
| `--rate <hz>` | Frecuencia de muestreo de captura (predeterminado: 48 kHz, u otra frecuencia que anuncie el dispositivo si no tiene 48 kHz) |
| `--bits <16\|24>` | Profundidad de bits (predeterminado: 24 si el códec la admite) |
| `--max-packet <bytes>` | Tamaño máximo del paquete multimedia, 679-1679, predeterminado 1023 (consulte [Tamaño máximo del paquete multimedia](#max-packet)) |
| `--sbc-blocks <n>`, `--sbc-subbands <n>`, `--sbc-alloc <method>`, `--sbc-bitpool <n>` | Estructura de las tramas de SBC / SBC XQ: bloques 4/8/12/16 (predeterminado 16), subbandas 4/8 (predeterminado 8), asignación `loudness` (predeterminado) o `snr`, bitpool (predeterminado: según la calidad) |
| `--sbc-mode <mode>` | Modo de canal de SBC: `joint_stereo` (predeterminado), `stereo`, `mono` (el canal dual es `-c sbcxq`) |
| `--afh <auto\|off\|6,11>` | Clasificación de canales AFH del host (consulte [Calidad del enlace y AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Mostrar lo que recibe el receptor par; sin `-d`, transmitir a ese receptor (consulte [Prueba con receptor par](#receiver)) |
| `--scan [seconds]` | Mostrar los dispositivos cercanos y salir (predeterminado: 10 segundos) |
| `--probe` | Guardar en `logs\probe\` lo que el adaptador responde a comandos de lectura y salir, para adaptadores que BlueCaster aún no puede usar (consulte [Añadir compatibilidad con chips](chip-support)) |
| `--script <file>`, `--allow-writes` | Ejecutar un script de chip en el adaptador, guardar su informe y salir; `--allow-writes` permite ejecutar las líneas que pueden cambiar el chip (consulte [Añadir compatibilidad con chips](chip-support#chip-scripts)) |
| `-h` | Mostrar la ayuda |

Para desarrollo y pruebas también existen `--hci-tcp <host:port>` (un controlador virtual por TCP en lugar del adaptador WinUSB), `--test-tone` (1 kHz a la izquierda / 1.5 kHz a la derecha en lugar del audio capturado), `--duration <s>` (detenerse tras ese número de segundos), `--sessions <n>` (transmitir n veces seguidas, como Desconectar / Conectar en la aplicación), `--restart` (con `--sessions`: reiniciar Bluetooth entre ellas), `--restart-on-link` (volver a iniciar la sesión en cuanto su enlace está activo, como hace un ajuste cambiado durante la conexión), `--wait-off` (apagar los auriculares no termina la ejecución) y `--hci-capture <file.pklg>` (captura HCI de toda la ejecución); consulte [Compilación](building#emu). `-h` muestra las opciones de LE Audio (consulte [LE Audio](#le-audio)). `--services` muestra, una vez iniciado el streaming, los servicios que el dispositivo indica por SDP.

Sin `--cli`, `bluecaster-core.exe` es el programa residente: también inicia la ventana, salvo con `--background` (solo el icono de la bandeja; lo usa **Iniciar con Windows** cuando **Iniciar en la bandeja** está activado). `bluecaster-core.exe --quit` pide a un BlueCaster en ejecución que detenga la transmisión y se cierre (lo usa el instalador).

## Modos de captura {#capture-modes}

BlueCaster admite tres modos de captura de audio. Cada uno graba un punto distinto del recorrido del audio:

| Modo | Qué graba |
|:-----|:----------------|
| Loopback del sistema | Lo que reproduce el dispositivo de salida predeterminado de Windows: todas las aplicaciones mezcladas |
| Dispositivo virtual | Lo que las aplicaciones reproducen en el dispositivo seleccionado (p. ej., VB-CABLE) |
| Aplicación | Solo lo que reproduce una aplicación (y sus procesos secundarios), después del procesamiento propio de la aplicación. Requiere Windows 11 o Windows 10 versión 2004 (compilación 19041) o posterior |

**Dispositivos de audio** muestra la misma explicación en **Origen de salida**. Un dispositivo nuevo captura con Loopback del sistema, que funciona sin instalar nada más; elija allí Dispositivo virtual o Aplicación para él.

El Loopback del sistema copia el audio; no se lo quita al dispositivo de salida predeterminado, así que el audio también suena allí (altavoces, auriculares con cable, un televisor HDMI, ...). Para oírlo solo en los auriculares Bluetooth, use un dispositivo virtual como VB-CABLE.

En el modo de desarrollador, con **Desarrollador > Ajustes de la app > Usar el mezclador de volumen** activado, **Silenciar la salida predeterminada de Windows mientras hay conexión** (en la pantalla **Mezclador de volumen**; desactivado de forma predeterminada) silencia ese dispositivo durante la transmisión y lo restablece al terminar, para todos los orígenes de captura salvo el tono de prueba. Esto no afecta a lo que captura BlueCaster. La tecla de silencio de Windows silencia entonces los auriculares (se envía silencio; vuelva a pulsarla para reproducir) y el dispositivo sigue silenciado; las teclas de volumen, que en Windows también reactivan el sonido, lo dejan silenciado y los auriculares sonando. Con un dispositivo LE Audio, la tecla de silencio solo mantiene silenciado el dispositivo. Un dispositivo que ya estaba silenciado sigue así, y si BlueCaster se cierra de forma inesperada, el dispositivo queda silenciado: reactívelo en Windows. Con el modo de desarrollador desactivado, el silencio queda desactivado, diga lo que diga este ajuste.

Con ese interruptor activado, la pantalla **Mezclador de volumen** y el panel desplegable ajustan el volumen de cada aplicación, con o sin ese silencio. Funcionan con la salida silenciada, cuando el mezclador de Windows puede cortar el sonido. Si la salida predeterminada de Windows es un dispositivo virtual, silenciarla puede dejar mudo también el flujo: en ese caso, desactive el silencio.

En el modo Dispositivo virtual, **Usar como salida predeterminada de Windows mientras hay conexión** convierte el dispositivo seleccionado en la salida predeterminada de Windows durante la transmisión, de modo que las aplicaciones reproducen en él sin cambiar la configuración de Windows. Al detener la transmisión, BlueCaster vuelve al dispositivo predeterminado anterior.

- La lista **Origen de salida** ofrece, en Dispositivo virtual, solo los dispositivos que BlueCaster detecta como dispositivos de audio virtuales. Si no hay ninguno, **Dispositivos de audio** explica para qué hace falta uno y ofrece **Descargar VB-Cable**.
- El dispositivo de comunicación predeterminado (el que usan las aplicaciones de llamadas) no se cambia.
- Algunas aplicaciones siguen reproduciendo en el dispositivo anterior hasta que se reinician.
- Si BlueCaster se cierra inesperadamente, el dispositivo predeterminado no se restaura. Cámbielo en la configuración de Sonido de Windows.

### Modo Aplicación {#application-mode}

El modo Aplicación envía el audio de una aplicación, por ejemplo un reproductor de música, un navegador o una aplicación de efectos. Elija la aplicación en la lista **Origen de salida**, en Aplicación. Muestra las aplicaciones que están reproduciendo audio en ese momento; si la suya no aparece, inicie la reproducción en ella y haga clic en **Actualizar** junto a la lista. Se incluyen los procesos secundarios de la aplicación, así que también funciona un navegador que reproduce el audio desde un proceso aparte.

- No es necesario que la aplicación esté en ejecución. BlueCaster se conecta igualmente a los auriculares y empieza a enviar en cuanto la aplicación se inicia. Si la aplicación se cierra durante la transmisión, BlueCaster sigue conectado y la vuelve a captar cuando se reinicia.
- La aplicación sigue sonando también en su propio dispositivo de salida. Para oírla solo en los auriculares, silencie ese dispositivo de salida (su volumen maestro): esto no afecta a lo que captura BlueCaster. Pero no silencie la aplicación en el Mezclador de volumen de Windows, porque los auriculares también recibirían silencio.
- El control deslizante de la aplicación en el Mezclador de volumen de Windows cambia el nivel enviado a los auriculares (y el de su dispositivo de salida). El volumen maestro y las teclas de volumen no cambian el audio enviado; con **Sincronizar volumen de Windows** activado, cambian en su lugar el volumen de los auriculares (consulte [Volumen](#volume)).
- Windows convierte el audio a la frecuencia de muestreo que pide BlueCaster, así que el formato del dispositivo en la configuración de Sonido no lo limita.
- En Windows 10 anterior a la versión 2004 (compilación 19041), Windows no permite capturar por aplicación. Al conectar un dispositivo configurado con captura de Aplicación se muestra entonces un error que pide otro origen de salida.

### Volumen {#volume}

Durante la transmisión, el control deslizante **Volumen** de Inicio y del panel desplegable de la bandeja funciona cuando los auriculares admiten el volumen absoluto de AVRCP (si no, queda desactivado). Ajusta el volumen en los propios auriculares, de modo que el audio enviado conserva toda su resolución. El altavoz de al lado silencia (volumen 0 en los auriculares) y vuelve a activar el sonido (al volumen de antes). Con el volumen a 0, se haya ajustado como se haya ajustado (el altavoz, el control deslizante, Windows o los botones de los auriculares), BlueCaster envía silencio, ya que muchos auriculares siguen sonando débilmente a su volumen más bajo. El control deslizante también sigue los botones de volumen y los controles táctiles de los auriculares.

**Sincronizar volumen de Windows** (en **Dispositivos de audio**, para todos los dispositivos; activado de forma predeterminada) hace que un volumen de Windows y el volumen de los auriculares sean uno solo durante la transmisión. El volumen de Windows sincronizado es el del dispositivo que captura BlueCaster: el dispositivo de salida predeterminado en los modos Loopback del sistema y Aplicación, el dispositivo seleccionado en el modo Dispositivo virtual. Desactivado, el volumen de Windows y el de los auriculares son independientes.

- Los botones y gestos de volumen de los auriculares, y el control deslizante **Volumen**, mueven el volumen de Windows sincronizado, y su control deslizante en la configuración de Sonido mueve el volumen de los auriculares.
- Las teclas de volumen y el control flotante de volumen de Windows siempre actúan sobre el dispositivo de salida predeterminado, así que solo cambian el volumen de los auriculares cuando el dispositivo de salida predeterminado es el sincronizado (siempre, salvo en el modo Dispositivo virtual sin **Usar como salida predeterminada de Windows mientras hay conexión**).
- Al iniciar la transmisión, el volumen de Windows pasa a ser el de los auriculares; al detenerla, el dispositivo recupera el volumen que tenía antes.
- El audio se captura antes de aplicar el volumen de Windows, por lo que se reduce una sola vez, en los auriculares. Si el volumen sincronizado también cambia el audio capturado, el volumen se aplica dos veces: en ese caso desactive la sincronización (consulte [Aplicaciones de efectos para todo el sistema](#effects-apps)).
- El silencio no se sincroniza: silenciar el dispositivo de salida deja el PC en silencio, pero no silencia los auriculares.

Las versiones anteriores permitían que un dispositivo siguiera el volumen de otro dispositivo de salida (en modo de desarrollador); esa opción ya no se usa.

BlueCaster guarda el volumen propio de cada dispositivo de audio y lo vuelve a aplicar en cuanto se conecta, antes de enviar audio, así que el volumen de un dispositivo no pasa a otro.

### Botones de los auriculares {#headphone-buttons}

Los controles de reproducir/pausar, pista siguiente y pista anterior de los auriculares (botones o gestos táctiles, p. ej. un doble toque en los WH-1000XM4) controlan la reproducción en el PC durante la transmisión: BlueCaster los pasa a Windows como teclas multimedia, que llegan a la aplicación que aparece en los controles multimedia de Windows (Spotify, un navegador, un reproductor...). Detener también se transmite.

- Reproducir y pausar actúan ambos como la tecla Reproducir/Pausar de Windows, de modo que un toque siempre alterna entre reproducción y pausa, aunque la idea que tienen los auriculares del estado no esté actualizada.
- BlueCaster indica a los auriculares si se está reproduciendo audio (según haya o no audio que capturar), para que los auriculares que se pausan al quitárselos envíen el comando correcto.
- Avance rápido / retroceso (mantener pulsado un botón) no tienen tecla multimedia en Windows y se ignoran.
- Los gestos de volumen cambian el volumen propio de los auriculares; con **Sincronizar volumen de Windows**, el volumen de Windows sincronizado los sigue (consulte [Volumen](#volume)).
- Desactívelo con **Dispositivos de audio > Controlar la reproducción con los botones del dispositivo de audio** (un interruptor para todos los dispositivos), p. ej. si otro programa ya reacciona a las teclas multimedia y estas actúan dos veces.

Esto requiere AVRCP en los auriculares, que casi todos tienen. Sin él, la transmisión funciona como siempre, solo que sin los botones.

### Aplicaciones de efectos para todo el sistema {#effects-apps}

Una aplicación de efectos para todo el sistema (un ecualizador, un potenciador de sonido, una aplicación de sonido envolvente virtual, ...) suele instalar su propio dispositivo de salida virtual. Las demás aplicaciones reproducen en ese dispositivo; la aplicación de efectos procesa el audio y reproduce el resultado en el dispositivo de salida que usted elige en ella. El modo de BlueCaster que elija decide si los auriculares reciben los efectos:

| Configuración | Qué llega a los auriculares |
|:--------|:----------------------------|
| Dispositivo virtual: el dispositivo virtual propio de la aplicación de efectos | El audio **antes** de los efectos: lo que las aplicaciones envían a la aplicación de efectos |
| Aplicación: la aplicación de efectos | El audio **después** de los efectos: lo que reproduce la aplicación de efectos (recomendado) |
| Dispositivo virtual: el dispositivo de salida de la aplicación de efectos (p. ej., VB-CABLE configurado como su salida) | El audio después de los efectos (para versiones de Windows sin modo Aplicación) |

En el modo Aplicación, el dispositivo de salida de la aplicación de efectos puede ser cualquiera: BlueCaster toma lo que reproduce la aplicación, no lo que reproduce el dispositivo, así que el dispositivo solo es un destino para el audio. Para que el PC no reproduzca el audio al mismo tiempo, elija una salida que nadie escuche (p. ej., el audio HDMI de un monitor) o silencie el volumen maestro de ese dispositivo. Un dispositivo virtual como VB-CABLE también sirve como destino silencioso, pero no es necesario. El dispositivo debe estar habilitado y conectado.

Si el dispositivo virtual propio de la aplicación de efectos es el dispositivo de salida predeterminado de Windows, su volumen también reduce lo que recibe la aplicación de efectos. Con **Sincronizar volumen de Windows** activado, un cambio de volumen se aplica entonces dos veces (en la entrada de la aplicación de efectos y en los auriculares). Desactívelo y use el volumen de los auriculares o el control deslizante **Volumen**.

#### Por qué BlueCaster no tiene su propio dispositivo de salida virtual {#no-virtual-device}

Una entrada de auriculares Bluetooth en la lista de dispositivos de salida de Windows, que una aplicación de efectos pudiera seleccionar directamente, requeriría un controlador de audio en modo kernel. BlueCaster no incluye ninguno:

- Escribir y mantener un controlador de audio de kernel es un trabajo enorme, y un error en él puede bloquear Windows (pantalla azul).
- Windows 10 y 11 solo cargan controladores firmados por Microsoft. Obtener esa firma requiere un certificado de firma de código EV (varios cientos de dólares estadounidenses al año) y una cuenta de Microsoft Partner Center, normalmente registrada como empresa.
- La única forma de evitar la firma es el modo de firma de prueba de Windows, que reduce la seguridad del sistema y hace fallar algunos programas (p. ej., juegos con antitrampas). BlueCaster no va a depender de él.

El modo Aplicación, o un dispositivo virtual gratuito como VB-CABLE, cubre el mismo uso.

- Windows no tiene una API pública para cambiar el dispositivo predeterminado. BlueCaster usa la interfaz no documentada IPolicyConfig, como otras herramientas de cambio de audio (Windows 10 y 11).

{: .warning }
Loopback del sistema y Dispositivo virtual usan el modo compartido de WASAPI: con la frecuencia de muestreo en Automático (también con la selección automática), BlueCaster pide 48 kHz, u otra frecuencia que anuncien los auriculares si no tienen 48 kHz (primero 44.1 kHz), y WASAPI remuestrea a ella el **Formato predeterminado** del dispositivo. Para usar LDAC a 96 kHz, elija 96 kHz en los ajustes del códec; no hace falta cambiar el **Formato predeterminado** en **Configuración de sonido > Propiedades del dispositivo > Opciones avanzadas**.

## Comparación de códecs

| Códec | Ideal para | Inconveniente |
|:------|:---------|:----------|
| LHDC V5 | La tasa de bits y la frecuencia de muestreo más altas (hasta 1000 kbps, 192 kHz) | Pocos dispositivos; necesita una conexión estable |
| LDAC (HQ) | Máxima calidad de audio en muchos dispositivos Sony y orientados a Android | Mayor latencia, necesita una conexión estable |
| LDAC (ABR) | Calidad + estabilidad | La tasa de bits varía con la conexión |
| aptX Adaptive | Alta calidad con un modo de baja latencia, en dispositivos con chip Qualcomm | La tasa de bits varía con el enlace |
| Opus 🧪 | Eficiente, buena calidad a 256 kbps | Pocos receptores (Opus A2DP de Android, PipeWire) |
| aptX HD | Alta calidad, mayor compatibilidad de dispositivos | 576 kbps fijos |
| aptX | La mayor compatibilidad entre dispositivos aptX | 16 bits, 352/384 kbps |
| aptX LL | Juegos, vídeo (baja latencia) | Menor calidad de audio (16 bits) |
| AAC | Dispositivos Apple, buena eficiencia | Calidad moderada |
| SBC XQ | Mucho mejor que SBC en cualquier dispositivo cuyo SBC lo permita | Necesita canal dual y bitpool ≥ 29; ~385-552 kbps |
| SBC | Máxima compatibilidad | La menor calidad de audio |

La pantalla **Códecs** muestra la misma comparación como tres barras en cada tarjeta de códec.

## Familia aptX y aptX Adaptive {#aptx-compatibility}

BlueCaster admite tres códecs aptX codificados con [libopenaptx](https://github.com/pali/libopenaptx) y aptX Adaptive, codificado con el codificador de [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD y aptX Low Latency son experimentales.** Siguen las implementaciones de Android / PipeWire y superan las pruebas de ida y vuelta de codificación/decodificación, pero aún no se han verificado con auriculares reales.

{: .note }
**¿Solo necesita aptX clásico?** Windows 10 ya admite aptX clásico en su pila Bluetooth integrada (no aptX HD, aptX LL ni aptX Adaptive), así que BlueCaster no es necesario para ese códec. BlueCaster es útil sobre todo para LDAC, aptX HD y aptX Low Latency.

| Códec | Vendor ID / Codec ID | Encabezado RTP | Estado |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Sí | Compatible (265-427 kbps, 48 / 96 kHz, 24 bits) |
| aptX HD | 0x000000D7 / 0x0024 | Sí | Compatible (576 kbps, entrada de 24 bits) |
| aptX | 0x0000004F / 0x0001 | No | Compatible (352/384 kbps a 44.1/48 kHz) |
| aptX Low Latency | 0x0000000A o 0x000000D7 / 0x0002 | No | Compatible (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**aptX Adaptive está confirmado con auriculares reales a 48 kHz:** un usuario lo transmitió a unos EarFun Air Pro 4+ en 2.0.2-beta.6 sin cortes ni tirones, tan estable como el aptX clásico ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)). Además supera las pruebas unitarias, el conjunto de pruebas bit a bit del proyecto original y la prueba de extremo a extremo contra el receptor Bluetooth virtual (`tools/emu`, que comprueba el encabezado RTP, el tamaño de los paquetes y los tipos de trama; el audio no se decodifica, ya que no existe un decodificador abierto). El proyecto original informa de que funciona en Linux / PipeWire con Bose QuietComfort Ultra (1.ª generación) y B&W Px7 S2e a 48 kHz en Alta y Baja latencia. 96 kHz (Alta, 646 kbps) también está confirmado con auriculares reales ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)): se usa cuando se elige en los ajustes del códec (CLI `--rate 96000`), si los auriculares son un receptor R2 que lo anuncia (los ajustes solo lo ofrecen entonces), nunca en Baja latencia. Automático, también la selección automática, transmite a 48 kHz. [Se agradecen los informes](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11).

El codificador es el codificador aptX Adaptive en Rust puro de [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). El proyecto original lo describe como una reimplementación en sala limpia cuyas tablas numéricas se reconstruyeron a partir de binarios de Qualcomm distribuidos y obtenidos legalmente y de capturas por aire; no incluye código, SDK ni binarios de Qualcomm (consulte [Licencias](licenses)).

- **Cómo elegirlo**: Automático lo elige cuando los auriculares no ofrecen LHDC V5 ni LDAC. O elija **aptX Adaptive** en **Códecs**, o `-c aptxad`.
- **Formato**: 48 kHz estéreo, 24 bits; 96 kHz en Alta, si se elige, a un dispositivo que lo anuncie (aptX Adaptive R2); una solicitud de 44.1 kHz se captura a 48 kHz.
- **Auriculares**: receptores R1 (sin extensión de capacidades, p. ej. B&W Px7 S2e) y receptores R2 (flujo de bits R2).

| Calidad (**...** / `-q`) | Receptores R1 | Receptores R2 | Paquetes |
|:-------------------------|:---------|:---------|:--------|
| Alta (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 bytes |
| Baja latencia (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 bytes |

`ll` solo lo usa aptX Adaptive; los demás códecs lo tratan como Alta. Los niveles Estándar y Móvil (R1 364 / 318 kbps, R2 270 / 204 kbps) ya no se ofrecen como calidades: un dispositivo guardado con uno de ellos, y `-q sq` / `-q mq`, funcionan en Alta; ABR sigue pasando por ellos.

**ABR** (ajuste **ABR** del códec, CLI `-a`): en Alta, la tasa de bits baja un nivel (Estándar y después Móvil, no por debajo) mientras la cola de envío tiene 3 o más paquetes, como mucho cada ~0,5 s, y vuelve a subir un nivel (hasta Alta) tras ~3 s con como mucho un paquete en cola. Baja latencia no tiene ABR.

### No hay «recurso» de aptX Adaptive a aptX

Cuando BlueCaster se conecta, los auriculares anuncian una **lista de códecs** (extremos de transmisión AVDTP). BlueCaster elige un códec de esa lista. aptX Adaptive y aptX clásico son entradas distintas de la lista, así que dentro del códec nada «recurre» de uno al otro:

- **Los auriculares también ofrecen aptX clásico** -- Muchos auriculares aptX Adaptive lo hacen, porque Qualcomm promociona aptX Adaptive como retrocompatible con aptX / aptX HD. Automático elige aptX Adaptive (por encima de aptX HD y aptX); aptX clásico se usa cuando usted lo elige, como un códec independiente en la lista de los auriculares, no un modo reducido de aptX Adaptive.
- **Los auriculares solo ofrecen aptX Adaptive** (sin aptX clásico, aptX HD ni aptX LL) -- si elige aptX, se recurre al mejor códec que ofrezcan (en el orden del dispositivo en la pantalla **Códecs**), normalmente el propio aptX Adaptive.

Prioridad de la selección automática: LHDC V5 > LDAC > aptX Adaptive > Opus (experimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream y Opus 05, ambos experimentales, solo si se eligen)

### El códec seleccionado no está disponible {#codec-fallback}

Cuando solicita un códec concreto que los auriculares no ofrecen:

| Dónde | Comportamiento |
|:------|:---------|
| Aplicación (**Códecs**, selección automática desactivada) | Se conecta con el primer códec del orden del dispositivo (el orden de prioridad de Automático, salvo que haya arrastrado las tarjetas) que ofrezcan los auriculares, como haría Automático (Opus elegido, auriculares con aptX Adaptive, aptX, AAC y SBC: aptX Adaptive, no aptX, que está por debajo de Opus; con SBC XQ arrastrado al primer puesto: SBC XQ), y la insignia del códec indica durante unos segundos qué códec se usa en su lugar y por qué. Esto es para un códec elegido mientras los auriculares lo ofrecían: la pantalla **Códecs** muestra como elegido el que se usa. Una tarjeta elegida cuando no lo ofrecían (en su última conexión) se pide de todos modos, sin recurrir a otro: si la rechazan, la conexión se detiene con un error. |
| CLI (`-c <codec>`) | Se detiene con un error («El dispositivo no admite ...»); si solicitó aptX, aptX HD o aptX LL y los auriculares solo ofrecen aptX Adaptive, el mensaje lo indica. Añada `--fallback` para recurrir a otro como hace la aplicación, o use `-c auto`. |

Las versiones anteriores tenían en su lugar la opción **Usar otro códec compatible si este no está disponible**; ahora el recurso a otro códec está siempre activado. Algunas versiones preliminares de 2.0.2 tomaban el siguiente códec por debajo del elegido en lugar del mejor.

### Falta un códec mientras los auriculares están conectados a otro dispositivo {#sep-in-use}

Los auriculares ofrecen cada códec en un extremo de transmisión (SEP). En una conexión doble (multipunto), un extremo que transmite al teléfono o al PC se indica como **en uso**, y su códec no se puede usar hasta que esa transmisión se detenga. Algunos auriculares además solo ofrecen códecs de alta resolución (LDAC, L2HC, ...) mientras hay un único dispositivo conectado. BlueCaster registra entonces:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

y el error indica que hay extremos en uso por otro dispositivo. Desconecte los auriculares del teléfono / PC (o desactive la conexión doble en la aplicación de los auriculares) y vuelva a conectar. Un PC cuyo Bluetooth integrado siga emparejado con los auriculares también cuenta como otro dispositivo.

Los auriculares Sony como los WH-1000XM4 no ofrecen LDAC mientras **Conectar a 2 dispositivos simultáneamente** (multipunto) está activado: desactívelo en la aplicación Sony Sound Connect y vuelva a conectar. Algunos modelos, como los WH-1000XM6, admiten LDAC junto con multipunto.

La app de los auriculares (batería, cancelación de ruido, EQ) necesita su propio enlace con los auriculares, desde el teléfono o desde el Bluetooth integrado de este PC. Mientras está conectada, los auriculares están en conexión dual, y los cambios de códec en BlueCaster pueden tardar más o fallar. Desconecte el teléfono, o cierre la app, antes de cambiar de códec. [EncoDeck](works-with) controla los auriculares por el propio enlace de BlueCaster, así que no añade una segunda conexión.

### Frecuencia de muestreo

aptX, aptX HD y aptX LL solo admiten 44.1 kHz y 48 kHz (aptX Adaptive 48 kHz; 96 kHz si se elige y los auriculares lo anuncian). BlueCaster elige una de las frecuencias que anuncian los auriculares (en Automático 48 kHz, si no 44.1 kHz) y deja que WASAPI remuestree la captura. No es necesario cambiar la configuración de Sonido de Windows para aptX.

### Comprobar qué ofrecen sus auriculares

1. Conéctese a los auriculares.
2. Abra **Diagnóstico** y escriba `remote` en el filtro del registro.

Las mismas líneas están en los archivos de registro de la carpeta de configuración (`%APPDATA%\BlueCaster\logs\<YYYY-MM-DD>\core.log`; no están enmascarados, así que revíselos antes de adjuntar uno a un informe); en modo CLI se muestran en la consola. Las líneas de capacidades muestran exactamente qué códecs ofrecen los auriculares:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- aptX clásico está disponible (aunque también aparezca `aptX Adaptive=true`); Automático lo usa.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` con `aptX Adaptive=true` -- los auriculares solo ofrecen aptX Adaptive; elija aptX Adaptive, o Automático, AAC o SBC.
- `aptX Adaptive=true [not usable]` -- los auriculares ofrecen aptX Adaptive sin 48 kHz estéreo, que BlueCaster necesita.
- `remote vendor codec ... - unsupported` -- otro códec de fabricante que BlueCaster no implementa.

## Tamaño máximo del paquete multimedia (por dispositivo) {#max-packet}

**Tamaño máx. de paquete multimedia** de cada dispositivo (pantalla **Desarrollador**, un campo por dispositivo guardado; CLI: `--max-packet <bytes>`) fija el límite superior de los datos de audio de cada paquete Bluetooth, de 679 a 1679 bytes, para que los dispositivos de audio que necesitan otro valor puedan tener el suyo. La pantalla Desarrollador se muestra en modo de desarrollador (**Configuración > Modo de desarrollador**); la mayoría de los usuarios nunca lo necesita. El límite propio del dispositivo de audio (MTU) también se aplica siempre; BlueCaster usa el menor de los dos. El cambio se aplica a partir de la siguiente conexión. Antes era un ajuste del perfil (en **Avanzado**): un dispositivo conserva el valor del perfil que se mantuvo para él (consulte [Códecs](#codecs)); los perfiles guardados por la v1 anterior a la 1.1 toman el valor que entonces se configuraba de forma global.

**Se recomienda 1023 (predeterminado).** Cabe en un paquete de banda base Bluetooth (3-DH5). Los valores mayores solo ahorran un 1-2 % de sobrecarga, y cada paquete perdido supone entonces perder más audio. 679 cabe en un paquete 2-DH5 y es el mínimo que acepta LDAC.

## Verificar el flujo multimedia (bluecaster_decode) {#verify-stream}

La mayoría de los auriculares no pueden mostrar qué códec se está usando. **Diagnóstico > Iniciar captura HCI** / **Detener captura HCI** graba un registro de paquetes HCI (`logs\hci\hci_<date>_<time>.pklg` en la carpeta de configuración) con la negociación del códec y todos los paquetes multimedia enviados. Con LDAC a 990 kbps crece aproximadamente 0.5 GB por hora, así que capture solo el tiempo que necesite. En modo CLI, `--hci-capture <file.pklg>` graba toda la ejecución.

Puede iniciar una captura antes de conectar o durante la transmisión. Si se inicia durante una conexión, BlueCaster escribe primero los paquetes de establecimiento de la conexión que ha recordado (establecimiento del canal L2CAP y señalización AVDTP, incluida la negociación del códec), para que el archivo pueda analizarse igualmente. Diagnóstico muestra la ruta del archivo mientras graba y después de detenerla.

Abra la captura en Wireshark o compruébela con la herramienta para desarrolladores `bluecaster_decode` (se compila desde el código fuente; consulte [Compilación](building)):

```
bluecaster_decode "%APPDATA%\BlueCaster\logs\hci\hci_20260925_120000.pklg" -o C:\temp\check
```

La herramienta sigue la señalización AVDTP (SET_CONFIGURATION / RECONFIGURE y si los auriculares la aceptaron), extrae los paquetes multimedia que envió BlueCaster y los comprueba frente a la configuración negociada:

| Códec | Qué se comprueba | Salida |
|:------|:----------------|:-------|
| SBC, SBC XQ, FastStream, AAC, aptX, aptX HD, aptX LL, Opus, Opus 05 | Decodificación completa con los decodificadores de referencia; parámetros de trama frente a la configuración; número de tramas en el encabezado de la carga útil multimedia | `<prefix>.<n>.<codec>.wav` para escuchar |
| LDAC | Solo los encabezados de trama (palabra de sincronización, frecuencia de muestreo, modo de canal, longitud de trama), número de tramas, tasa de bits. No existe ningún decodificador LDAC de código abierto | Solo informe |
| LHDC V5 | Solo los encabezados de trama (número de tramas en el encabezado de la carga útil multimedia, números de secuencia), tasa de bits. No existe ningún decodificador de código abierto | Solo informe |
| aptX Adaptive | Solo su propio encabezado RTP (marca de tiempo por número de muestras, modo de latencia) y el encabezado de paquete del códec. No existe ningún decodificador de código abierto | Solo informe |

`bluecaster_decode --le-audio` hace lo mismo con una captura de LE Audio (LC3 sobre CIS).

Para cada flujo también informa de los saltos en la secuencia RTP, la velocidad a la que avanza la marca de tiempo RTP, la tasa de bits y cuántos paquetes ACL indicó el controlador Bluetooth como completados. La última línea es `RESULT: OK` o `RESULT: PROBLEMS FOUND` (código de salida 0 o 1).

{: .note }
Esto muestra lo que envió BlueCaster y que es válido para el códec negociado. Como un receptor solo puede decodificar el códec negociado, un audio correcto en los auriculares junto con un informe sin errores es una prueba sólida de que el códec se está usando de verdad.

## Calidad del enlace y AFH {#link-quality}

**Diagnóstico > Calidad del enlace** muestra lo que envía BlueCaster durante la transmisión: la tasa de bits real, los paquetes enviados, los paquetes descartados antes del envío (esperaron demasiado en la cola de envío: el enlace de radio no daba abasto), el audio descartado en la captura (el PC se quedó atrás), la cola de envío, el tiempo de radio usado y la **Señal respecto al ideal**: el RSSI de la conexión, relativo al rango óptimo de recepción del controlador (0 = bien, negativo = demasiado débil; no es un nivel absoluto; por debajo del rango se añade en dBm el nivel real máximo posible), y en cuántos de los 79 canales Bluetooth salta el enlace (AFH). Inicio y el panel desplegable de la bandeja lo resumen como Buena, Aceptable o Mala, o PC ocupado cuando el enlace está bien pero el PC descartó audio.

Bluetooth evita por sí mismo los canales ocupados (AFH), pero solo se entera de la presencia de Wi-Fi a través de los errores. BlueCaster además indica a su controlador qué canales debe evitar (clasificación de canales AFH del host), en cada conexión:

| AFH | |
|:--|:--|
| Automático (predeterminado) | Solo el canal Wi-Fi de 2.4 GHz al que está conectado este PC (se vuelve a comprobar cada 30 segundos mientras hay conexión). Nada si el PC no está en una Wi-Fi de 2.4 GHz: desconectado, en 5 GHz o por cable. Solo lee el número de canal, lo que no requiere permiso de ubicación; un canal de 6 GHz numerado del 1 al 14 cuenta como de 2.4 GHz |
| Desactivado | Nada: el controlador decide por su cuenta |
| Manual, p. ej. `6,11` | Estos canales Wi-Fi (1 a 14) |

Automático deja al AFH propio del controlador las redes que el PC solo oye: evitar el canal ocupado de un vecino resta 22 MHz al salto de frecuencia y puede empeorar el enlace en lugar de mejorarlo. Para evitar un canal así de todos modos, introdúzcalo en Manual.

Siempre quedan en uso al menos 20 canales Bluetooth. Configúrelo en la pantalla **Adaptador**, con `"afh"` en `settings.json` (carpeta de configuración) o con `--afh` en modo CLI. La línea de radio muestra qué canales Wi-Fi se evitan. Que sirva o no depende del entorno: mídalo con la prueba con receptor par.

### La pestaña Radio {#radio-tab}

**Diagnóstico > Radio** dibuja el mapa de canales que el adaptador informa para el enlace (Read AFH Channel Map), así se ve dónde hay interferencias incluso en un PC sin Wi-Fi. Los canales en rojo son los que el AFH del propio adaptador dejó fuera; cuando la mayor parte de un canal Wi-Fi está en rojo, la pestaña lo nombra. Los canales que BlueCaster pidió evitar (Automático o Manual, arriba) llevan un contorno.

En modo desarrollador, **Escanear** pide a Windows un escaneo Wi-Fi nuevo y dibuja cada red de 2,4 GHz sobre la banda, más alta cuanto más fuerte. En Windows 11 24H2 y posteriores la lista cuenta como datos de ubicación: Windows puede pedir acceso a la ubicación y, sin **Permitir que las aplicaciones de escritorio accedan a la ubicación**, no da la lista. El escaneo no cambia AFH: Automático sigue evitando solo el canal del propio PC (ver arriba por qué).

### Combinado (experimental) {#afh-blend}

En modo desarrollador, **Evitar canales Wi-Fi** ofrece además **Combinado**, con **Suave**, **Medio** o **Estricto**. Evita lo que evita Automático y, además, los canales Bluetooth en los que dos señales apuntan a interferencias: las redes Wi-Fi que se oyen a menos de 11 MHz (cuentan desde -90 dBm, del todo a -50 dBm) y la frecuencia con la que el propio adaptador dejó fuera el canal en sus mapas de canales recientes. Una red fuerte que el adaptador sigue usando sin problemas no basta por sí sola con Suave o Medio. Cuanto más estricto, antes se evita un canal y menos quedan siempre en uso: 56 (Suave), 48 (Medio) o 40 (Estricto) de 79.

Mientras Combinado está activo, BlueCaster busca redes Wi-Fi cada 2 minutos, lo que necesita acceso a la ubicación igual que **Escanear**; sin él decide solo el mapa del adaptador. Combinado aún no se ha medido con adaptadores reales y puede empeorar el enlace respecto a Automático: compárelo con la [prueba con receptor par](#receiver) antes de quedárselo. Sin modo desarrollador funciona como Automático. En `settings.json` y con `--afh` es `blend-mild`, `blend-medium` o `blend-strong`.

## Latencia {#latency}

Lo que oye va por detrás del PC la suma de varios retardos. En el PC: el período de 10 ms del motor de audio, las rondas de 10 ms del codificador y las tramas propias del códec (las 1024 muestras de AAC son 21 ms, los paquetes de LDAC unos 5 ms), y la cola de envío, que se mantiene casi vacía con un buen enlace (**Diagnóstico > Calidad del enlace** la muestra). La mayor parte está en el dispositivo de audio: almacena el flujo antes de reproducirlo, para superar los cortes de radio, y almacena más con las tasas de bits de LDAC y LHDC V5. Esa parte es la misma sea lo que sea lo que envía el audio, un teléfono incluido.

Cuando un dispositivo admite Delay Reporting, comunica ese retardo a BlueCaster una vez configurado el flujo y de nuevo cuando cambia; **Diagnóstico > Calidad del enlace** lo muestra como **Retardo del dispositivo (informado)** (y **Copiar diagnóstico** lo incluye). Un dispositivo que no lo admite muestra un guion. Android usa ese mismo informe para retrasar el vídeo de modo que siga sincronizado con el sonido; Windows no tiene tal mecanismo, así que un reproductor de vídeo mantiene su imagen por delante del sonido ese retardo. La mayoría de los reproductores pueden retrasar el audio o el vídeo una cantidad elegida (un ajuste de desfase o retardo de audio): póngalo en el retardo informado, o pruebe valores hasta que la sincronización labial sea correcta. A los juegos no se les puede ayudar así: una tasa de bits más baja (LDAC Media o Baja, un códec con tramas más pequeñas, Baja latencia de aptX Adaptive, aptX LL) hace que algunos dispositivos almacenen menos.

El **Límite de la cola de envío** del modo de desarrollador (**Desarrollador > Ajustes de códec**) es un límite de cuánto puede esperar un paquete antes de descartarse, no un retardo en sí: con un buen enlace, ningún paquete se le acerca. Solo importa cuando la radio se detiene, donde un límite más corto cambia un corte en el sonido por menos retardo después.

## Prueba con receptor par (tools/linux_sink) {#receiver}

La vista de calidad del enlace muestra lo que envía BlueCaster. Para ver lo que llega realmente, ejecute `tools/linux_sink/bluecaster_sink.py` en un segundo PC con Linux. Controla directamente el adaptador Bluetooth normal de ese PC (sirve el integrado) mediante Bumble sobre el canal de usuario HCI, como receptor A2DP para todos los códecs incluido LDAC, mide cada paquete multimedia que llega y ofrece las estadísticas por la red:

Copie la carpeta `tools/linux_sink` al PC con Linux (basta con la carpeta) y después:

```bash
sh setup.sh      # una vez: bibliotecas de decodificación, entorno de Python (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # iniciar el receptor (root: toma el control del adaptador Bluetooth)
```

En el PC con Windows, cierre BlueCaster (bandeja > **Salir**) y ejecute:

```bash
bluecaster-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` encuentra el receptor en la red local, transmite a su adaptador Bluetooth salvo que `-d` indique otro dispositivo, e imprime las estadísticas del receptor cada 5 segundos y al final: tasa de bits, paquetes perdidos y tardíos (números de secuencia RTP), jitter y huecos, cortes y saltos de un búfer de reproducción modelado, errores de flujo / decodificación y el RSSI en el receptor (relativo a su rango óptimo de recepción: 0 = bien, negativo = demasiado débil). Sin `--test-tone` envía el audio del sistema. `--remote-sink <host[:port]>` indica el receptor en lugar de buscarlo.

La misma prueba funciona en la ventana: con **Configuración > Modo de desarrollador** activado, abra **Desarrollador > Prueba con receptor par**. Deje **Receptor** vacío para encontrarlo en la red local (o escriba su dirección); **Conectar** se conecta a sus estadísticas; elija el códec, sus ajustes, la fuente (**Tono de prueba** o **Audio del sistema**) y, si hace falta, la MTU de L2CAP y el búfer de reproducción que ofrece el receptor, y después **Iniciar**. Comprueba cada paso y muestra lo que envió este PC junto a lo que contó el receptor. No hace falta cerrar nada antes: la ventana usa el BlueCaster en ejecución.

Ambos lados cuentan los paquetes de un flujo desde su primer paquete, de modo que se pueden comparar los paquetes enviados y recibidos: deberían coincidir. Bluetooth retransmite dentro de los controladores los paquetes de radio perdidos, y BlueCaster no abandona un paquete mientras el enlace está activo, así que un enlace de radio deficiente se manifiesta como huecos, jitter y cortes en el receptor, no como paquetes perdidos; cuando el enlace no da abasto, los paquetes se descartan antes del envío en el lado emisor. Ninguno de los dos lados puede conocer el número de retransmisiones.

Mientras la herramienta se ejecuta, la pila Bluetooth del propio receptor no puede usar su adaptador. La búsqueda usa una difusión UDP en el puerto 51201, así que ambos PC deben estar en el mismo segmento de red (si no, escriba la dirección). Consulte `tools/linux_sink/README.md` para ver todas las opciones y el protocolo de estadísticas.

## Servicios y claves de emparejamiento {#shared-pairing}

En el modo de desarrollador, un dispositivo clásico guardado tiene una etiqueta **Servicios** en **Dispositivos de audio**. Abre:

- los servicios que anuncia el dispositivo, en una tabla de **Nombre** (el nombre Bluetooth o, si no, el propio del dispositivo; **Servicio desconocido** para los de un fabricante que no tienen ninguno) y **GUID**. Se consultan mientras el dispositivo está conectado; si no lo está, se muestra la lista de su última conexión desde que se inició BlueCaster
- debajo, **Clave de emparejamiento**: la clave de enlace que BlueCaster guarda para el dispositivo y su tipo. **Copiar clave** la copia, por ejemplo para dar a otro sistema del mismo PC el mismo emparejamiento con el mismo adaptador. Un dispositivo emparejado por Bluetooth clásico no tiene claves LE (Long Term Key, Identity Resolving Key). Cualquiera con la clave puede hacerse pasar por el dispositivo: no la compartas

**Importar emparejamientos de Windows**, en **Dispositivos de audio** y en el paso de dispositivos de la configuración (un adaptador con WinUSB; se muestra mientras Windows tenga emparejado a través de él un dispositivo que BlueCaster no ha guardado), toma las claves de enlace que el Bluetooth de Windows guardó para ese adaptador antes de asignarle WinUSB: los auriculares emparejados con Windows a través de él se conectan a BlueCaster sin volver a emparejarlos. Windows pide permiso de administrador, porque solo el sistema puede leer esas claves. Un dispositivo para el que BlueCaster ya tiene clave conserva la suya (se emparejó con BlueCaster después, la clave de Windows es la antigua). Las claves que Windows creó con otro adaptador, como el Bluetooth integrado del PC, no sirven: un emparejamiento pertenece a la dirección del adaptador. Windows no guarda el tipo de una clave, así que una clave importada recibe el tipo 4. Los dispositivos LE no se importan. Los auriculares importados se añaden a los dispositivos guardados con los nombres que conoce Windows, listos para conectar.

## LE Audio (experimental) {#le-audio}

BlueCaster también puede transmitir a auriculares LE Audio (LC3 sobre flujos isócronos conectados, BAP unicast), con el volumen del auricular (VCP), los botones multimedia (MCP), los dos auriculares de un par (CSIP) y un modo de baja latencia para auriculares gaming (GMAP). Esta función es **experimental** y se elige por dispositivo: solo se usa para la entrada LE Audio de un dispositivo. Solo se ha probado con un emulador de Bluetooth, **todavía no con auriculares LE Audio reales**, y puede que no funcione con los suyos; para el uso diario, use el A2DP clásico.

Qué se necesita:

- Un adaptador Bluetooth USB con **Bluetooth 5.2 o posterior que admita canales isócronos (ISO)**, con WinUSB asignado como para A2DP. Un adaptador sin ISO se rechaza con «adapter does not support LE Audio (no ISO buffers)».
- Auriculares compatibles con LE Audio (LC3); algunos necesitan activar antes LE Audio en su propia aplicación.

Cómo usarlo:

1. Ponga los auriculares en modo de emparejamiento y busque dispositivos en **Dispositivos de audio**. Los dispositivos LE Audio llevan la etiqueta **LE Audio**, un par de auriculares **Conjunto LE Audio** (elija cualquiera de los dos: BlueCaster encuentra el otro). Unos auriculares que también admiten A2DP aparecen dos veces, una por tipo.
2. En **Códecs**, elija la tarjeta LC3 de la entrada LE Audio. No se elige códec ni calidad (BlueCaster elige la mejor configuración LC3 que aceptan los auriculares) y solo están disponibles las capturas Loopback del sistema y Dispositivo virtual.
3. Debajo, **Baja latencia para dispositivos de audio gaming (experimental)** está activada por defecto: con los auriculares que admiten el rol de juego de GMAP usa tramas LC3 más cortas; los demás auriculares reciben la configuración multimedia en ambos casos.

No hay ningún interruptor de LE Audio en Configuración. Si el lado LE llegara a perturbar un adaptador, `"le_audio_experimental": false` en `settings.json` (**Configuración > Carpeta de configuración**) lo desactiva por completo.

En el modo CLI, `--le-audio -d <dirección>` transmite a un dispositivo LE Audio, `--le-device <dirección>` añade otro dispositivo del grupo (el segundo auricular), `--le-scan` lista los dispositivos LE Audio cercanos, y `--le-low-latency` / `--le-preset <nombre>` eligen la configuración (`bluecaster-core.exe --cli -h` muestra todas las opciones LE).

## Actualizaciones {#updates}

BlueCaster consulta GitHub Releases como máximo una vez al día (**Configuración > Buscar actualizaciones al iniciar**; **Incluir versiones preliminares al buscar actualizaciones** también ofrece versiones beta). Cuando hay una versión nueva, un cuadro de diálogo muestra unas líneas sobre las novedades (las notas completas están a un enlace), con **Actualizar ahora** (**Abrir la página de descarga** en una copia portable) y **Más tarde**; **Más tarde** la deja en Configuración hasta la siguiente versión. Configuración también lo indica:

- **Copia instalada**: **Actualizar ahora** descarga la actualización firmada. Si hay un dispositivo de audio conectado, espera y la instala cuando se desconecta, de modo que una actualización nunca corta el audio. BlueCaster se cierra entonces, instala la nueva versión y se vuelve a iniciar por sí solo.
- **Copia portable** (o una compilación sin firma de actualizaciones): **Abrir la página de descarga** abre la página de la versión.

## Solución de problemas

Un sonido que se corta o se entrecorta, una conexión que falla o una tasa de bits menor de la esperada: consulte **[Cortes y problemas de conexión](wireless)**, que resume en qué han resultado consistir los informes recibidos hasta ahora y qué comprobar primero.

### La ventana indica que el núcleo no se está ejecutando

La ventana necesita `bluecaster-core.exe` a su lado y lo inicia por sí misma. «El núcleo de BlueCaster no se está ejecutando» significa que no se pudo iniciar: reinstale BlueCaster (o vuelva a extraer el zip portable). «El núcleo es de otra versión» significa que los dos programas proceden de versiones distintas: instale la misma versión de ambos (la última versión publicada lo consigue).

### Falla la asignación o la restauración del controlador

- «Se rechazó la solicitud de administrador»: no se cambió el controlador. Vuelva a hacer clic en el botón y acepte la solicitud.
- «Error al cambiar el controlador»: consulte **Diagnóstico > Registro** para ver los detalles.
- «El adaptador no volvió a su controlador anterior»: si el adaptador se configuró con Zadig, es posible que Windows vuelva a elegir el paquete de controlador WinUSB de Zadig. Elimine ese paquete de controlador en el Administrador de dispositivos (adaptador > Desinstalar el dispositivo, con «Intentar quitar el controlador de este dispositivo»), y después desconecte y vuelva a conectar el adaptador.
- «El controlador se restauró, pero Windows usa un solo adaptador Bluetooth a la vez»: otro adaptador Bluetooth (como el integrado del PC) está en uso en Windows. Para usar este en Windows en su lugar, deshabilite el otro en el Administrador de dispositivos y, después, desconecte y vuelva a conectar este.
- Puede que los dispositivos emparejados en BlueCaster no se conecten en Windows una vez que el adaptador vuelve a su controlador: quítelos en la configuración de Windows y vuelva a emparejarlos.

### El adaptador Bluetooth no responde {#adapter-no-answer}

Si el adaptador no devuelve nada después de abrirlo (no responde a HCI Reset), BlueCaster desiste a los 5 segundos, lo abre una vez más y, si sigue sin responder, muestra «El adaptador Bluetooth no responde». Desconecte el adaptador, vuelva a conectarlo e inténtelo de nuevo (o pulse **Reiniciar Bluetooth**). No hace falta reiniciar BlueCaster: un inicio fallido libera el adaptador y cada nuevo inicio lo vuelve a inicializar.

Para evitar que un adaptador se quede bloqueado, BlueCaster restablece sus canalizaciones USB al abrirlo y envía HCI Reset al cerrarlo.

### Problemas de emparejamiento tras actualizar

BlueCaster empareja mediante SSP (Just Works) con **General Bonding**, y las claves de enlace se guardan para que los auriculares puedan reconectarse sin volver a emparejar (la v2 lee las claves de enlace de la v1). Si emparejó los auriculares con una compilación antigua de BlueCaster y ya no se conectan, elimine el emparejamiento de BlueCaster en los auriculares (consulte el manual de los auriculares para borrar la lista de emparejamientos) y vuelva a emparejarlos.

### Tiempos de espera de conexión {#connection-timeouts}

BlueCaster registra registros SDP (A2DP Source, AVRCP Controller / Target) para que los auriculares lo identifiquen como fuente de audio. Si un intento de conexión agota el tiempo de espera, la conexión semiabierta se cierra, de modo que puede volver a intentarlo sin reiniciar BlueCaster. Asegúrese de que los auriculares estén encendidos, dentro del alcance y no conectados a otro dispositivo.

Una conexión fallida se reintenta automáticamente (hasta 3 intentos) cuando el enlace se estableció y luego no transmitió nada, y cuando los auriculares no respondieron. Si los auriculares ya no reconocen el emparejamiento (se restablecieron o se emparejaron con otro PC), se descarta la clave guardada y se vuelve a emparejar una vez. El error indica entonces qué ha ocurrido:

| Mensaje | Significado |
|:--|:--|
| El dispositivo no respondió | Apagado, fuera de alcance o no disponible para conexión (si es nuevo: modo de emparejamiento) |
| La conexión se estableció, pero no transmitió nada | Condiciones de radio: vuelva a intentarlo o cambie de sitio el adaptador / el dispositivo |
| El dispositivo respondió, pero luego cerró la conexión por sí mismo | Ahora no acepta conexiones: se está apagando, se está cargando o está conectado a otro dispositivo |
| Error de autenticación, también tras volver a emparejar | Ponga el dispositivo en modo de emparejamiento, o elimine su entrada de la lista de emparejamientos, y vuelva a intentarlo |

**La primera conexión** con unos auriculares necesita el modo de emparejamiento; las siguientes se reconectan con la clave de enlace guardada. Si falla aunque estén en modo de emparejamiento, puede que sigan conectados a un teléfono u otro PC (también mediante LE Audio): desconéctelos allí, desactive LE Audio en la app de los auriculares si tiene esa opción, póngalos en modo de emparejamiento y vuelva a conectar. El registro muestra entonces `signaling connection failed (0x66)` (rechazado por seguridad: los auriculares conservan la clave de un emparejamiento anterior, por ejemplo con el Bluetooth de Windows de este PC) o `(0x67)` (rechazado por falta de recursos: ocupados con otro dispositivo).

Una conexión perdida se restablece automáticamente (hasta 10 intentos). Si el PC entra en suspensión durante la transmisión, BlueCaster vuelve a conectar al reanudarse; estos intentos no se cuentan.

Desactive **Dispositivos de audio > Reconectar automáticamente** si cambia los auriculares entre el PC y un teléfono apagándolos y encendiéndolos. Una conexión perdida queda entonces desconectada y BlueCaster suelta el adaptador Bluetooth, para que los auriculares puedan conectarse al teléfono. Vuelva a conectarlos al PC con **Conectar** en Inicio.
