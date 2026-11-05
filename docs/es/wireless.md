---
title: Cortes y problemas de conexión
layout: default
parent: Inicio
nav_order: 2.4
---

# Cortes y problemas de conexión
{: .no_toc }

Un sonido que se corta, se entrecorta o se detiene, una conexión que no llega a establecerse o una tasa de bits menor de la esperada. Esta página resume en qué han resultado consistir esos informes, para que compruebe las causas habituales antes de informar, e indica qué enviar si no es ninguna de ellas.

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

## Compruebe esto primero {#checklist}

Bluetooth comparte la banda de 2.4 GHz con la Wi-Fi, los ratones y teclados inalámbricos, los receptores de auriculares inalámbricos y cualquier otra radio Bluetooth de la habitación. Un adaptador USB tiene una antena pequeña, y unos auriculares de botón una aún más pequeña, con su cabeza entre ellos y el PC. La mayoría de los informes de «mala conexión» recibidos hasta ahora se debían a una de estas causas; cada una tarda un minuto en probarse. El formulario de informe de errores tiene una casilla para cada una: marque las que haya probado.

{: .note }
Pruébelas de una en una y escuche unos minutos entre una y otra: si los cortes desaparecen, ha encontrado la causa.

### 1. Apague los demás adaptadores Bluetooth de este PC

El Bluetooth propio de Windows (el adaptador integrado) sigue transmitiendo mientras BlueCaster transmite por el adaptador USB que tiene al lado, y los dos comparten una banda y a menudo la misma zona de antena. También es donde se conecta la app de los auriculares en el PC, lo que da a los auriculares un segundo enlace (consulte el punto siguiente). Desactive el adaptador integrado en el Administrador de dispositivos mientras BlueCaster está en uso, o al menos para la prueba. BlueCaster muestra un aviso en Inicio cuando detecta otro adaptador activo.

Un segundo adaptador activo es una de las dos causas que un probador veterano señaló para las conexiones inestables y los cambios de códec.

### 2. Deje a los auriculares una sola conexión

La conexión doble (multipunto) permite a los auriculares estar conectados a la vez a un teléfono y al PC. Cada conexión cuesta tiempo de radio, y algunos auriculares además ofrecen menos códecs, rechazan el enlace manos libres (así que no hay nivel de batería) o reservan un extremo de códec para el otro dispositivo mientras está activa. La app complementaria de los auriculares también cuenta: mantiene su propio enlace abierto, en el teléfono o en el Bluetooth de este PC, aunque solo la use para la batería o el ecualizador.

Para la prueba: desactive la conexión doble en la app de los auriculares, apague el Bluetooth del teléfono, cierre la app complementaria en el teléfono y en el PC, y vuelva a conectar desde BlueCaster.

La conexión doble es la otra causa que señaló ese probador; los cambios de códec se volvieron mucho más fluidos al desconectar la app complementaria. Una primera conexión fue rechazada mientras los auriculares seguían conectados a un teléfono con LE Audio activado, y varios auriculares de botón ofrecieron a BlueCaster menos códecs o una tasa de bits menor mientras intervenía un teléfono o su app. El LDAC de Sony está desactivado mientras el multipunto de los auriculares está activado.

### 3. Cambie de sitio el adaptador

- Use un **puerto USB 2.0**, o un cable alargador USB corto. Los puertos y cables USB 3 radian ruido en la banda de 2.4 GHz, justo donde está el adaptador.
- Manténgalo **lejos de otros receptores inalámbricos**: receptores de ratón y teclado gaming, receptores de auriculares inalámbricos, antenas Wi-Fi. Un receptor en el puerto contiguo es una causa frecuente de exactamente este patrón.
- Manténgalo **a la vista de los auriculares**. Un PC bajo la mesa, con usted y la mesa en medio, cuesta mucha señal. Un cable alargador USB hasta la mesa ayuda más que ninguna otra cosa.

Un receptor de ratón gaming junto al adaptador, en un PC sin adaptador Wi-Fi que guiara el AFH, dejó un enlace que transportaba unos 750 kbps y un adaptador que marcó como malos hasta 46 de los 79 canales. Unos auriculares de botón a un metro del adaptador seguían mostrando una señal débil, suficiente para un apagón cada pocos minutos cerca del límite de la tasa de bits.

### 4. Mantenga la Wi-Fi de 2.4 GHz lejos del enlace

Ponga este PC y, si puede, el teléfono y los demás dispositivos cercanos en una Wi-Fi de 5 o 6 GHz. Si la de 2.4 GHz tiene que seguir activa, BlueCaster indica al adaptador que evite el canal al que está conectado este PC (AFH, Automático). No puede ver la Wi-Fi cuando el PC no tiene adaptador Wi-Fi, va por cable o está en otra banda distinta de la red de 2.4 GHz del router: en ese caso, averigüe el canal del router (su página de administración, o una app de análisis Wi-Fi en el teléfono), fíjelo allí si cambia de canal e introdúzcalo en **Adaptador > Evitar canales Wi-Fi > Manual**, por ejemplo `6` u `11`. Consulte [Calidad del enlace y AFH](usage#link-quality).

LDAC 990 kbps perdió unos 1.700 paquetes en 5 minutos en una red de 2.4 GHz muy cargada, y 33 en 15 minutos una vez fijado el router en un canal y puesto el AFH en Manual con ese canal.

### 5. Deje margen al enlace

Un enlace Bluetooth transporta poco más de 1 Mbps en el mejor de los casos; LDAC 990 kbps, LHDC V5 a 900 y 1000 kbps y SBC XQ con un bitpool alto no dejan casi nada para las retransmisiones. Los auriculares true wireless reenvían el audio de un auricular al otro, lo que también cuesta tiempo de radio, así que 900 kbps o más les resulta difícil incluso con un enlace limpio.

- Pruebe **ABR** (tasa de bits adaptativa; LDAC, LHDC V5, aptX Adaptive, bitpool adaptativo para SBC), o un nivel menos: LDAC 660, LHDC V5 500, SBC XQ bitpool de 29 a 36.
- Compruebe si los cortes persisten con **SBC o AAC**. Si es así, no es la tasa de bits.
- Como comprobación de los auriculares: emparéjelos con un teléfono, ponga allí el mismo códec y la misma tasa de bits y escuche en el mismo sitio. Si allí también se cortan, el límite está en su lado.

En un enlace que transportaba 750 kbps, todo lo que superaba 500 kbps se cortaba, mientras que ABR no tuvo ningún corte. SBC XQ por encima del bitpool 36 se quedó en silencio en unos auriculares de botón económicos cuya capacidad SBC permitía 53. En los mismos auriculares, LHDC V5 se cortaba al azar donde LDAC era estable. Un enlace limpio sí lo transporta: el adaptador integrado de un portátil transmitió LDAC 990 kbps durante 5 minutos con 0 paquetes perdidos.

### 6. Actualice BlueCaster

Varios tirones que parecían de radio eran la aplicación. 2.0.1 enviaba el audio más rápido que en tiempo real cuando un cable virtual lo entregaba tarde, lo que los auriculares oyen como un segundo de sonido y otro de silencio; una transmisión tras Desconectar > Conectar se entrecortaba hasta reiniciar la aplicación; una pila Bluetooth obsoleta se negaba a conectar tras la suspensión, una sesión larga o muchos reinicios. Todo está corregido en la versión actual ([Actualizaciones](usage#updates)). Si el problema desaparece tras actualizar, no hace falta ningún informe.

## Cómo leer Calidad del enlace {#link-quality}

**Diagnóstico > Calidad del enlace** (y el gráfico de Inicio) distingue entre la radio, el PC y la aplicación. Haga una captura de pantalla mientras ocurre el problema y adjúntela al informe.

| Qué ve | Qué significa | Apunta a |
|:--|:--|:--|
| **Señal respecto al ideal** muy por debajo de 0 (unos -25 dB o menos) | La señal de los auriculares llega al adaptador muy por debajo de su rango ideal. No es un nivel absoluto: 0 es el ideal, -30 son 30 dB por debajo, así que -30 aún puede reproducir pero sin margen | Radio: distancia, su cabeza y su cuerpo, colocación del adaptador |
| **Cola de envío** llena, **paquetes descartados antes del envío** en aumento, tasa de bits por debajo de la del códec | Los paquetes esperaron demasiado a la radio; el enlace no puede transportar esta tasa de bits ahora mismo | Radio o interferencias, o una tasa de bits demasiado alta |
| **Salto de frecuencia (AFH)** dando saltos, o fijos en 79 mientras se pierden paquetes | El adaptador marca canales como malos por los errores que ve en ellos; en 79 ha dejado de distinguirlos | Interferencias: Wi-Fi, otros receptores, un segundo adaptador Bluetooth |
| **Audio descartado en la captura** en aumento, Inicio indica **PC ocupado** | El PC no entregó el audio a tiempo | El PC: la carga, el dispositivo de captura, un cable virtual |
| Todo plano y vacío, pero silencio o tirones | El enlace está bien y en el PC no se descarta nada | La aplicación o el decodificador de los auriculares: infórmelo con el paquete de diagnóstico |

El lado receptor no se puede ver desde el PC. Para medir lo que llega de verdad, la [prueba con receptor par](usage#receiver) usa un PC con Linux como si fueran los auriculares.

## Lo que deciden los auriculares {#headphones}

Algunos informes de «solo me da 400 kbps» o «falta LDAC» no tienen nada que ver con el enlace: los auriculares indican a BlueCaster qué códecs y tasas de bits aceptan, y BlueCaster lo respeta, como hace Android. La línea de registro `remote supports ...` muestra lo que ofrecieron.

- Los **Huawei FreeBuds** ofrecen LDAC a un PC solo después de que su app AI Life haya activado «priorizar la calidad de sonido» con la multiconexión desactivada; si no, solo ofrecen SBC y AAC. Desde 2.0.2, BlueCaster les pide ese modo por sí mismo cuando se elige LDAC con la selección automática desactivada.
- Los **auriculares LHDC V5** anuncian una tasa de bits máxima. En el modo de 96 kHz de su app, un modelo anunciaba 400 kbps; en su modo de 192 kHz, 1000 kbps. Los OPPO Enco Free4 y Enco X4 anuncian 400 kbps y 48 kHz a un PC diga lo que diga la app; 2.0.2 pide Hi-Res a los auriculares OPPO por su propio protocolo cuando LHDC V5 transmite con ABR desactivado, y vuelve a conectar una vez para ello; aún sin confirmar con auriculares reales.
- **ABR** mantiene LHDC V5 en 400 kbps o menos, como en Android.
- Un códec que los auriculares **reservan para otro dispositivo** falta mientras ese dispositivo está conectado ([Falta un códec mientras los auriculares están conectados a otro dispositivo](usage#sep-in-use)).

Cómo ver lo que ofrecen: [Comprobar qué ofrecen sus auriculares](usage#codec-fallback).

## La conexión falla {#connecting}

El error de Inicio indica qué ha pasado; la tabla de [Tiempos de espera de conexión](usage#connection-timeouts) explica cada mensaje. Los casos de radio:

- **El dispositivo no respondió** (Page Timeout): apagado, fuera de alcance, no está en modo de emparejamiento para una primera conexión, o el adaptador sigue con su firmware ROM. Un adaptador Realtek sin su firmware no encuentra nada en una búsqueda y agota el tiempo de espera en cada conexión, o se abre bien y luego nunca transmite; un archivo de firmware resultó ser una página web guardada. La pantalla Adaptador muestra si el firmware está cargado.
- **La conexión se estableció, pero no transmitió nada**: condiciones de radio, o los auriculares están ocupados con otro dispositivo. Vuelva a intentarlo, cambie de sitio el adaptador.
- **Error de autenticación**, o el registro dice `signaling connection failed (0x66)`: los auriculares conservan la clave de un emparejamiento anterior, normalmente el del Bluetooth de Windows de este mismo PC. `(0x67)`: están ocupados con otro dispositivo, por ejemplo un teléfono con LE Audio activado. Desconecte los demás dispositivos, desactive LE Audio en la app de los auriculares, póngalos en modo de emparejamiento y conecte una vez; las conexiones siguientes usan la clave guardada.
- Los auriculares **no pueden conectarse al teléfono** tras Desconectar: 2.0.2 suelta el enlace al Desconectar; desactive **Reconectar automáticamente** si los pasa del PC al teléfono apagándolos y encendiéndolos.

## En qué resultaron consistir los informes {#reports}

Todos los informes recibidos hasta ahora sobre el lado inalámbrico, agrupados según lo que resultaron ser al final.

| Causa | Informes | Ejemplos |
|:--|:--|:--|
| **Entorno**: la radio, la colocación, otro dispositivo | Alrededor de un tercio | Wi-Fi de 2.4 GHz en los mismos canales, un receptor de ratón gaming junto al adaptador, un segundo adaptador Bluetooth activo, conexión doble y una app complementaria en ejecución, una tasa de bits sin margen en auriculares true wireless, una señal débil de unos auriculares de botón pequeños, un archivo de firmware que era una página web guardada |
| **Auriculares**: lo que ofrecen a un PC | Alrededor de un cuarto | LDAC solo tras el modo de calidad de sonido de Huawei, LHDC V5 limitado a 400 kbps por el modo propio de los auriculares, Hi-Res configurado por el protocolo de OPPO, un códec reservado para el teléfono |
| **Aplicación**: un error de BlueCaster, todos corregidos | Menos de la mitad | Audio enviado más rápido que en tiempo real con un cable virtual, tirones tras Desconectar > Conectar, una pila Bluetooth obsoleta tras la suspensión o muchos reinicios, el enlace mantenido tras Desconectar, auriculares que conectan primero confundidos con una clave incorrecta, una caché de códecs que ocultaba LDAC, adaptadores no reconocidos (un chip, una clase USB, una vinculación de controlador que aún no estaba activa) |

Lo que destaca:

- **Los tirones y los cortes se reparten casi a partes iguales** entre la aplicación (errores de temporización en 2.0.1 y las primeras betas de 2.0.2) y el entorno. Los dos casos de interferencias que se midieron muestran la misma imagen: la tasa de bits enviada cae a entre 550 y 800 kbps, la cola de envío se llena y el mapa de canales del adaptador se va a 79 de 79.
- **Los problemas de conexión fueron sobre todo la aplicación**, y casi todos en un mismo camino: los auriculares abriendo ellos mismos la conexión tras encenderse. El emulador de Bluetooth no puede ejercitar ese camino, así que estas correcciones se apoyan en los registros de los usuarios.
- **«Falta un códec» y «tasa de bits bloqueada» eran los auriculares** decidiendo qué ofrecer a un PC, normalmente dirigidos por su app complementaria.
- Algunos problemas **parecían de radio pero no lo eran**: silencio con AAC en un modelo (el formato de trama), la salida de Windows que se quedó en el cable virtual después de apagar los auriculares, unos segundos de sonido por los altavoces mientras un cambio de códec vuelve a conectar, sonido también en unos auriculares con cable (así funciona el Loopback del sistema).

## Cómo informar {#reporting}

Si las comprobaciones anteriores no lo han encontrado, abra un [informe de error](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/new?template=bug_report.yml) y marque lo que haya probado. Lo que resuelve rápido un informe sobre el lado inalámbrico:

1. **Diagnóstico > Informar de un problema**, **Registro detallado** en **Detallado (debug)**, y después reproduzca el problema.
2. El **paquete de diagnóstico** (**Paquete de diagnóstico > Guardar…** con **Incluir capturas HCI** activado, después **Informar en GitHub**). Contiene las líneas de estado de la transmisión (cola de envío, paquetes descartados, señal, canales AFH a lo largo del tiempo) y la captura HCI de una conexión fallida, que muestran dónde se detuvieron los paquetes.
3. Una **captura de pantalla de Calidad del enlace** mientras ocurre.
4. Si también ocurre con **SBC o AAC**, y con una **tasa de bits menor**.
5. Sus respuestas a las preguntas del formulario sobre Conexión doble, Otro adaptador Bluetooth y Wi-Fi.
