---
title: Añadir compatibilidad con chips
layout: default
parent: Inicio
nav_order: 3.5
---

# Añadir compatibilidad con chips

BlueCaster controla por sí mismo el adaptador Bluetooth (WinUSB), así que tiene que hacer lo que hace el controlador de Windows del fabricante antes de que el adaptador funcione: en muchos chips, cargarle el firmware en cada inicio. Los adaptadores Realtek, Intel, Broadcom y CSR ya funcionan, y los MediaTek MT7925, MT7922, MT7921 y MT7902 y los Qualcomm QCA61x4, QCA9377 y WCN6855 de forma experimental; los demás chips de MediaTek (MT7927 y otros más antiguos) y de Qualcomm (QCA6290, WCN785x) no, porque nadie ha escrito todavía su cargador. `app/src/crates/bt-host/src/mediatek.rs` (el de los MT79xx) es un cargador real del que partir para otro chip de MediaTek.

Esta página describe cómo añadir uno. Necesita un adaptador con ese chip; los mantenedores no lo tienen. Aunque no escriba código, puede ayudar: los pasos 1 y 2 dan el informe que necesita quien lo escriba.

{: .note }
El diseño que hay detrás y lo que sigue pendiente están en la [nota para desarrolladores](../chip-bringup-dev).

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Qué necesita

- El adaptador, conectado a un PC con Windows 10 / 11
- BlueCaster 2.0.2 o posterior (los pasos siguientes solo necesitan la aplicación)
- Para escribir el cargador: un entorno de compilación como en [Compilación](building), y [USBPcap](https://desowin.org/usbpcap/) con [Wireshark](https://www.wireshark.org/) para grabar lo que hace el propio controlador de Windows

## Paso 1: Ceder el adaptador a BlueCaster

1. **Configuración > Modo de desarrollador**: activado
2. **Adaptador**: en la tarjeta del adaptador, **Reemplazar por el controlador WinUSB** (Windows solicita permiso de administrador). Para un chip que BlueCaster todavía no puede usar, este botón solo está disponible en modo de desarrollador
3. Si la tarjeta indica **Reconecta o reinicia Windows**, hágalo

Mientras el adaptador tenga WinUSB, el Bluetooth de Windows no puede usarlo. En un portátil cuyo único Bluetooth sea ese chip, los teclados, ratones y auriculares conectados a través de Windows dejan de funcionar hasta que devuelva el controlador: **Restaurar el controlador original** en la misma tarjeta.

## Paso 2: Leer lo que dice el chip (sondeo del chip)

En la tarjeta del adaptador, sección **Firmware**: **Leer datos del chip**. La pila Bluetooth se detiene un momento, se ejecuta el sondeo y se abre el Explorador con el informe, `probe-<date>_<time>.json` en `logs\probe\` de la carpeta de configuración (`%APPDATA%\BlueCaster`). Se conservan los 10 más recientes.

El sondeo solo lee. No envía ningún HCI Reset ni ningún comando de fabricante:

| Solicitud | Qué indica |
|:--|:--|
| Read Local Version Information | Versión HCI / LMP, el fabricante (ID de empresa), la subversión LMP: a menudo la versión de la ROM o del firmware |
| Read BD_ADDR | La dirección del adaptador (cero o un valor provisional en algunos chips antes de que se ejecute su firmware) |
| Read Local Supported Commands / Features, Read Buffer Size | Lo que ofrece el código en ejecución: una ROM suele ofrecer mucho menos que el firmware |
| Read Local Name | Algunas ROM indican su propio nombre |
| LE Read Local Supported Features, LE Read Buffer Size | Lo mismo para LE |
| Descriptor de dispositivo USB, cadenas de fabricante y de producto | Solicitudes USB estándar por la canalización predeterminada |

También registra la configuración USB que indica Windows (interfaces, endpoints) y cualquier evento que el chip haya enviado por su cuenta. Una solicitud sin respuesta también queda registrada: un chip que no responde a nada hasta que se carga su firmware ya es un dato en sí.

Desde la línea de comandos, con el VID:PID o el ID de instancia del adaptador (los muestra la tarjeta del adaptador):

```bat
"%LOCALAPPDATA%\BlueCaster\bluecaster-core.exe" --cli -u 0e8d:7961 --probe
```

Antes, cierre BlueCaster desde la bandeja, ya que los dos no pueden usar el adaptador a la vez.

**Enviar un informe**: después de **Leer datos del chip**, la tarjeta del adaptador muestra lo que se leyó y un botón **Informar en GitHub**. Este abre una incidencia nueva con la versión, el adaptador, ese resumen y su compilación de Windows ya rellenados, y muestra el archivo del informe en el Explorador: arrástrelo a la incidencia y añada la procedencia del adaptador (si va integrado en un portátil: el modelo). El informe contiene la dirección Bluetooth y el nombre del adaptador; no contiene nada sobre sus auriculares. Un **Paquete de diagnóstico** (pantalla **Diagnóstico**) con las capturas HCI incluidas también lleva los informes de sondeo, en `probe/`.

## Paso 3: Grabar lo que hace el controlador de Windows

Un sondeo muestra el estado del chip, no cómo cambiarlo. Eso lo sabe el controlador de Windows del fabricante: grábelo una vez y el cargador repetirá lo que hace.

1. **Restaurar el controlador original** en la tarjeta del adaptador, para que el controlador de Windows vuelva a encargarse de él
2. Instale USBPcap y Wireshark; inicie Wireshark como administrador y capture la interfaz USBPcap en la que está el adaptador. `USBPcapCMD.exe` (en la carpeta de USBPcap) lista los dispositivos de cada concentrador raíz; el adaptador integrado de un portátil está en un concentrador interno, a menudo listado solo como «Dispositivo compuesto USB» o por su VID:PID. Si no está seguro, capture todas las interfaces USBPcap a la vez
3. Haga que el controlador cargue el firmware: desconecte el adaptador y vuelva a conectarlo; si es integrado, **Deshabilitar dispositivo** y **Habilitar dispositivo** en el Administrador de dispositivos. Un firmware que sobrevive a un reinicio en caliente puede cargarse solo después de un apagado completo
4. Detenga la captura cuando el adaptador aparezca en la configuración de Bluetooth de Windows. Busque el descriptor de dispositivo del adaptador (su VID:PID) entre los primeros paquetes, filtre por su dirección (`usb.bus_id == <bus> && usb.device_address == <address>`) y guarde la captura

Qué buscar, por orden:

- **Transferencias de control** antes de cualquier comando HCI: `bmRequestType` 0x40 / 0xC0 (de fabricante, OUT / IN) con sus `bRequest`, `wValue`, `wIndex`. Suelen leer un ID de chip o reiniciar una parte del chip
- **Comandos HCI** (solicitudes de clase, `bmRequestType` 0x20) con opcodes de 0xFC00 a 0xFFFF: comandos de fabricante. Una larga serie del mismo opcode con parámetros grandes es el envío del firmware; sus respuestas indican cómo se confirma cada parte
- **Eventos** que no son Command Complete / Status: eventos de fabricante (código 0xFF, o un código propio del fabricante como 0xE4), por ejemplo «firmware iniciado», o la confirmación de cada parte del firmware en lugar de un Command Complete
- Datos **Bulk OUT** antes del primer HCI Reset: algunos chips reciben el firmware por la canalización ACL
- De dónde sale el archivo: compare los datos enviados con los archivos de la carpeta del controlador (Administrador de dispositivos > detalles del controlador, o `C:\Windows\System32\DriverStore\FileRepository\`) y con los archivos del chip en [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) (`mediatek/`, `qca/`), que pueden redistribuirse y que los usuarios de BlueCaster pueden descargar

Anote cualquier espera que el controlador haga entre pasos (las marcas de tiempo de la captura): un chip que todavía se está reiniciando no responde.

{: .warning }
Trabaje a partir de capturas, informes de sondeo y documentación del fabricante. No copie ni siga de cerca código GPL, como los controladores `btmtk` o `btqca` del kernel de Linux: BlueCaster tiene licencia MIT, y un cargador derivado de ellos no podría incluirse.

## Probarlo sin compilar: scripts de chip {#chip-scripts}

Antes de escribir un cargador, pruebe en el adaptador lo que mostró la captura: un **script de chip** es la secuencia en forma de texto, una solicitud por línea, que se ejecuta en lugar de la pila Bluetooth. Alguien sin el adaptador puede escribir uno y enviárselo a quien lo tenga; el resultado vuelve como informe, sin una compilación nueva.

**Ejecutar uno**: modo de desarrollador, **Desarrollador > Chip**: elija el adaptador (con WinUSB), escriba el script o abra un archivo, **Ejecutar**. La respuesta a cada línea se muestra debajo, y **Mostrar informe** abre `script-<date>_<time>.json` en `logs\probe\` (el script va incluido). Desde la línea de comandos: `bluecaster-core --cli -u <adapter> --script <file>`.

```text
# una solicitud por línea; los números son hexadecimales (0x opcional) salvo wait, timeout= y chunk=
hci 1001                           # un comando HCI: opcode y después los bytes de parámetros
hci fc05 ff
bulk fc09 00 01 02                 # lo mismo por la canalización bulk OUT
usb in vendor device 63 0 0 4      # control IN: tipo, destinatario, solicitud, valor, índice, longitud
usb out vendor device 64 0 0 01    # control OUT: ..., y después los bytes de datos
wait 100                           # milisegundos
expect e4 02 01 timeout=5000       # un evento: su código y sus primeros bytes de parámetros
bootloader off                     # modo bootloader (activado desde el inicio)
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # lo mismo por la canalización bulk OUT
```

- `hci` y `bulk` esperan el Command Complete o el Command Status del comando (2 s) y lo registran, sea cual sea su estado
- `upload` envía un archivo de la carpeta de firmware en comandos de ese opcode, cada uno con los bytes de cabecera y hasta `chunk` bytes del archivo (240 de forma predeterminada), y espera cada confirmación: `ack=complete` (predeterminado: un Command Complete o Command Status del opcode con estado 0) o `ack=event:<code>[:<bytes>]`. Un estado de error lo detiene
- El script se detiene en la primera línea que falla (sin respuesta, un error, un archivo que falta) e indica cuál
- Los eventos que no responden a ninguna línea también quedan en el informe

**Seguridad**: solo las solicitudes que se sabe que leen se ejecutan tal cual: las lecturas HCI estándar (parámetros informativos y de estado, Read Local Name, las lecturas LE), las solicitudes USB estándar de descriptores, `wait`, `expect` y `bootloader`. Un script con cualquier otra línea solo se ejecuta si usted lo permite (**Permitir ejecutarlas** debajo de la lista de esas líneas; `--allow-writes` en la línea de comandos). Los comandos de fabricante que pueden escribir en una memoria que el chip conserva tras apagarse y encenderse nunca se ejecutan, permita lo que permita (`denied_opcodes` en `app/src/crates/bt-host/src/chips.rs`: de momento, el BCCMD de CSR). El modo bootloader está activado desde el inicio, así que al cerrar después el adaptador no se envía ningún HCI Reset. El firmware cargado en la RAM de un chip desaparece al desconectar y volver a conectar el adaptador.

## Paso 4: Escribir el cargador

Las rutas de aquí en adelante son relativas a la raíz del repositorio. Importan tres crates:

| Crate (paquete) | Carpeta | Qué contiene |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | El host Bluetooth: cargadores, el sondeo, la detección de chips |
| `bluecaster-core` (biblioteca) | `app/src/crates/core` | Configuración y adaptadores: qué cargador se ejecuta |
| `bluecaster-core-exe` | `app/src/core` | El ejecutable del servicio (`bluecaster-core.exe`): las solicitudes de la ventana, las notas de los adaptadores |

Los dos últimos llevan «core» en el nombre; los nombres de paquete de arriba son los que acepta `cargo -p`.

Un cargador es una máquina de estados sin E/S propia: recibe los eventos del controlador Bluetooth, los resultados de sus solicitudes USB y su temporizador, y devuelve **acciones** que ejecuta la aplicación. Así se puede probar sin el chip y los detalles de USB y de hilos quedan fuera de él.

1. Copie `app/src/crates/bt-host/src/bringup/sample.rs`, un cargador completo para un chip inventado con sus pruebas, a `app/src/crates/bt-host/src/<vendor>.rs` (por ejemplo `mediatek.rs`). Añada `pub mod <vendor>;` a `app/src/crates/bt-host/src/lib.rs`, en orden alfabético con los demás módulos
2. Mantenga los nombres: el tipo es `Loader` (usado como `<vendor>::Loader`), `vendor()` devuelve el nombre del fabricante. Deje las pruebas en el archivo, como hace el ejemplo
3. Sustituya todo lo que el ejemplo marca como "made up" por lo que mostraron los pasos 2 y 3: la comprobación de versión, la solicitud USB, el nombre del archivo, el comando de descarga y cómo se confirma, cómo se inicia el firmware
4. Reescriba el comentario del módulo al principio: qué necesita el chip, los pasos en orden y de dónde sale cada dato (su captura del controlador de Windows, documentación del fabricante, informes de sondeo). No cite el código fuente de otros controladores como origen de un paso

### La interfaz

Todo está en `app/src/crates/bt-host/src/bringup.rs`.

| Método de `ChipLoader` | Cuándo se llama |
|:--|:--|
| `vendor()` | Para las líneas del registro ("MediaTek") |
| `start()` | Una vez, con el adaptador abierto, antes de la inicialización HCI |
| `on_event(event)` | Para cada evento HCI: código, longitud, parámetros |
| `on_control(result)` | Con el resultado del último `UsbControl`: los bytes leídos, o por qué falló |
| `on_timeout()` | Cuando vence el temporizador del último `Timer` |
| `missing_file_hint(file)` | Para el registro cuando el cargador terminó con `MissingFile`: qué poner y dónde ("x.bin from linux-firmware (mediatek/)") |
| `step()`, `progress()` | Tras cada lote de acciones: el nombre del paso ("download") y cuánto lleva (comandos enviados del total), para la pantalla Adaptador mientras se inicia el adaptador |

Después de `Done`, la aplicación descarta el cargador: una respuesta tardía nunca le llega.

| `Action` | Qué hace |
|:--|:--|
| `Command(bytes)` | Envía un comando HCI (opcode, longitud, parámetros) por la canalización de control; queda en la captura HCI |
| `BulkCommand(bytes)` | Envía un comando, con los mismos bytes que `Command`, por la canalización bulk OUT, donde algunos bootloaders reciben el firmware |
| `UsbControl(request)` | Una transferencia de control por la canalización predeterminada (registros del fabricante); se hace al momento y `on_control` recibe el resultado antes de la siguiente acción. Queda en la captura HCI como nota |
| `Bootloader(on)` | Para un chip cuyo cargador responde por la canalización bulk IN, o que no debe recibir un HCI Reset (algunos se bloquean con él); consulte los patrones. Un chip con ROM que responde a HCI con normalidad no lo necesita |
| `Timer(ms)` | Inicia o reinicia el único temporizador del cargador |
| `Done(Ok(outcome))` | Terminado: sigue la inicialización HCI. `AlreadyRunning`, `Loaded { firmware, detail }`, `Rom` (`Probed` es propio del sondeo) |
| `Done(Err(error))` | El adaptador no se puede usar: `MissingFile(name)`, `BadFile { file, why }`, `Command { opcode, status }`, `Timeout(step)`, `Rejected(code)`, `BadVersion`, `UnsupportedVariant(v)`, `Unsupported(why)`, `Host(why)` |

Funciones auxiliares, para que un cargador no tenga que analizar bytes a mano:

- `hci_command(opcode, params)` construye un comando
- `parse_event(event)` devuelve `HciEvent::Complete { opcode, status, ret }` (los parámetros de retorno después del estado), `Status { opcode, status }`, `Vendor(params)` (evento 0xFF) u `Other { code, params }` (cualquier otro código, incluido uno propio del fabricante, como 0xE4)

**Archivos de firmware**: el cargador recibe un `FileReader` y le pide un archivo por su **nombre, sin carpeta** (`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`). La aplicación lo busca en la carpeta de firmware (`%APPDATA%\BlueCaster\firmware`) y, dentro de ella, en la subcarpeta del fabricante (`firmware\mediatek\`), tal como los organiza linux-firmware; el paso 6 define esa subcarpeta.

### Patrones

El ejemplo muestra cada uno de ellos; quite lo que su chip no necesite.

- **Un comando, una confirmación**: envíe, inicie el temporizador y, en el estado de espera, acepte solo la confirmación de ese opcode: un Command Complete, o un Command Status con estado 0 para un comando cuyo resultado llega después. Todo lo demás se ignora, para que una respuesta tardía no haga avanzar al cargador
- **Confirmado con un evento de fabricante**: algunos chips responden a cada parte del firmware con su propio evento en lugar de un Command Complete. Entonces ese evento es la confirmación: espere solo a él (su código y los parámetros que lo identifican), ignore en ese estado un Command Complete o un Command Status con estado 0 de ese comando y, aun así, termine con un estado de error en cualquiera de los dos. El ejemplo trata un Command Complete y un Command Status como la misma confirmación, así que para esto su `on_event` necesita un `match` sobre el estado y el evento
- **Varios archivos** (un parche y después una configuración): cargue y compruebe todos antes de enviar ninguno, envíelos en orden y nombre el último en `Loaded { firmware, .. }` (las líneas del registro nombran los demás)
- **Una pausa deliberada** (el controlador espera 100 ms antes del siguiente paso): un estado para ella y `Timer(100)`; `on_timeout` en ese estado envía el siguiente paso en lugar de fallar. En las pruebas, `SimRun::timeouts` también cuenta esas pausas
- **Un comando con dos respuestas** (una confirmación y después un evento de fabricante como «firmware iniciado»): dos estados, el segundo con su propio temporizador. Los estados `Launching` y `Starting` del ejemplo lo hacen. Si el evento lleva un código de resultado, uno incorrecto termina el cargador con `Rejected(code)`
- **Un estado de error** en un Command Complete o un Command Status termina el cargador con `Command { opcode, status }`
- **Modo bootloader**: `Bootloader(true)` antes del primer comando, incluso antes de comprobar la versión. Si todo sale bien (`Loaded` y también `AlreadyRunning`), envíe `Bootloader(false)` justo antes de `Done`, para que la inicialización HCI funcione con normalidad; tras un fallo, déjelo activado, ya que el chip sigue en su bootloader. La función `finish()` del ejemplo lo hace
- **Ya en ejecución**: si la versión o un registro muestran el firmware operativo, termine con `AlreadyRunning` enseguida. El adaptador conserva su firmware hasta que pierde la alimentación, y una segunda descarga puede fallar
- **Comprobar antes el archivo**: analice y compruebe el archivo entero antes de enviar nada (`BadFile` con el motivo), e indique exactamente qué archivo falta
- **Solo su chip**: si el cargador cubre solo algunos chips de un fabricante, compruebe que el adaptador es uno de ellos antes de enviar nada (su propio registro de ID de chip, o los ID USB con `ControlRequest::device_descriptor()`) y, si no, termine con `Unsupported`
- **Registre** con nivel `info` lo que averigüe del chip (ID de chip, versión de la ROM, el archivo elegido): es lo que muestran los informes
- No escriba nunca en la memoria flash ni en la OTP del chip, y lea archivos solo a través del `FileReader`

## Paso 5: Probarlo con un controlador simulado

Las pruebas usan `app/src/crates/bt-host/src/bringup/sim.rs`. Usted escribe un pequeño chip simulado (`impl SimController`) que responde a los comandos y a las transferencias de control como lo hizo el real en su captura, y `sim::run(&mut loader, &mut chip)` ejecuta el cargador exactamente como lo hace la aplicación, temporizador incluido. El resultado (`SimRun`) contiene el desenlace, cada comando enviado con su canalización (`commands[i].bulk`), cada transferencia de control, el modo bootloader al final (`bootloader`) y cuántas veces venció el temporizador. `sim::command_complete`, `command_status`, `vendor_event`, `event` (cualquier código), `opcode` y `params` construyen y leen los bytes; responda a los comandos bulk en `bulk_command`.

Cubra al menos, como hace el ejemplo:

- la descarga completa, comprobando los comandos enviados (cantidad, orden, tamaños, canalización)
- el modo bootloader al final de cada caso, si el cargador lo usa
- el firmware ya en ejecución
- un archivo que falta y un archivo dañado (sin enviar nada de él)
- un comando rechazado (estado de error), un controlador que no responde (tiempo de espera agotado) y cada espera que pueda agotarse
- una transferencia USB fallida, si el cargador usa alguna

Los bytes copiados de su captura son los mejores datos de prueba. Ejecute las pruebas con:

```bat
cargo test -p bt-host
```

## Paso 6: Integrarlo en la aplicación

Todos los archivos que hay que cambiar, para un fabricante ya listado como no compatible (MediaTek, Qualcomm) o para uno nuevo. Para un fabricante nuevo, el compilador señala los `match` que necesitan su variante; el resto, y todo lo de un fabricante ya listado, solo lo encuentra esta lista.

**¿Un chip o todo el fabricante?** Un cargador suele cubrir solo algunos chips de un fabricante (el MT7921, pero no el MT7925). Entonces solo esos chips pueden dejar de ser "unsupported": añada sus VID:PID a `LOADER_CHIPS` en `app/src/crates/bt-host/src/chips.rs` y deje el `support_tag()` del fabricante como está. `chips::chip_supported(vendor, vid, pid)` y `chip_support_tag()` responden entonces por adaptador, y la etiqueta del nombre del adaptador, su nota en la pantalla **Adaptador**, que se ofrezca WinUSB allí y la advertencia de «no implementado» siguen lo que respondan. Cambie `support_tag()` solo para un cargador que cubra todos los chips del fabricante (como el de Intel).

{: .note }
Mantenga los finales de línea de cada archivo: algunos archivos fuente usan CRLF y otros LF. Algunos editores y `sed -i` en Git Bash los reescriben sin avisar, y `cargo fmt` no los restaura. `git diff --stat` lo delata: un archivo que cambió en pocos sitios cuenta todas sus líneas. En ese caso, vuelva a convertir el archivo (por ejemplo con el selector de final de línea de la barra de estado de VS Code) antes de hacer el commit.

**Detección** (`app/src/crates/bt-host/src/chips.rs`):

1. `ChipVendor`: un fabricante nuevo recibe una variante (con una línea de documentación) y un brazo en `name()`. Para cualquier fabricante, actualice el comentario de documentación del enum (qué fabricantes tienen cargador) y la línea de documentación de la variante ("not supported")
2. Algunos chips: sus VID:PID en `LOADER_CHIPS`, y una prueba en el módulo del cargador de que su propia comprobación de chip (ID de chip, versión) acepta exactamente lo que dicen esas entradas. Todo el fabricante: `support_tag()` pasa de `"unsupported"` a `"experimental"`
3. `vendor_for_usb()`: los VID USB del fabricante. Un VID que también usan otros fabricantes debe comprobarse como `VID if pid == PID`, nunca entero: módulos de portátil con el VID del fabricante del módulo (AzureWave 0x13D3, Foxconn 0x0489), y VID compartidos como pid.codes 0x1209. Cada VID listado aquí también hace que `is_bluetooth_vendor_vid` trate los dispositivos de ese fabricante como adaptadores Bluetooth. Un VID:PID de la tabla Realtek de BTstack o de `OEM_TABLE` (ambas en `resolve_realtek_pid` de este archivo) se toma como Realtek antes de consultar a `vendor_for_usb`: compruebe que el suyo no está en ninguna de las dos
4. `company_name()`: el ID de empresa del Bluetooth SIG que devuelve Read Local Version, si aún no está
5. Pruebas: `app/src/crates/bt-host/tests/chips.rs` (los VID:PID y la etiqueta)

**Abrir el adaptador**: los dispositivos de clase Bluetooth (clase E0/01/01) se abren tal cual. Un chip que en su ROM indica una clase de fabricante (FF) también necesita sus VID:PID en `BUILTIN_VENDOR_CLASS` de `app/src/crates/core/src/adapter.rs`; la línea `usb.interfaces` del informe de sondeo muestra la clase.

**Iniciar el cargador**, como hace Intel (busque `intel_loader`):

6. `app/src/crates/bt-host/src/lib.rs`: un campo de `HostConfig` como `intel_loader` (con su valor predeterminado en `HostConfig::new`, la desestructuración en `BtHost::launch` y la línea de depuración), y la rama de `BtHost::launch` que instala el transporte y llama a la función de inicio de la aplicación en lugar de `setup_hci_and_power_on`, solo cuando no se usa H4 (`&& !tcp`, como la de Intel: el controlador del emulador no es un chip real). Se ejecuta un cargador por inicio: si están activados los indicadores de dos fabricantes (los dos adaptadores con WinUSB), gana la primera rama, así que pruebe solo con su adaptador en WinUSB
7. `app/src/crates/bt-host/src/app.rs`: una función `start_<vendor>_loader` como `start_intel_loader` (con `use crate::<vendor>;`), que da al cargador `read_firmware_file(&dir, "<vendor>", name)` (el nombre de la subcarpeta como en linux-firmware) y llama a `start_loader`
8. `app/src/crates/core/src/adapter.rs`, `configure()`: active el campo cuando un adaptador del fabricante con `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)` tenga WinUSB y no esté en uso (como `config.intel_loader`, con el registro "in use by another program" de Intel para los demás), y añádalo a la línea de depuración `adapter:` de esa función. La advertencia "firmware loading for this chip is not implemented" que hay debajo ya sigue a `chip_supported`
9. `app/src/crates/core/src/service.rs`, `host_config()`: añada el campo a la línea de depuración

**Lo que muestra la ventana**:

10. `app/src/core/src/state.rs`, `vendor_note()`: un brazo con una clave nueva `firmware.vendor_<vendor>`, fuera del brazo `MediaTek | Qualcomm`, y su prueba. Antes de eso, la función devuelve `firmware.vendor_unsupported` para los chips que `chip_supported` rechaza, así que los demás chips del fabricante la conservan. La pantalla **Adaptador** ofrece WinUSB a todos los adaptadores cuya nota no sea `firmware.vendor_unsupported`
11. `app/lang/*.json` (los 8 idiomas, en la misma línea en cada uno): la clave nueva, como la de Intel ("… adapter (experimental). …"). Si nombra un botón o una carpeta, use las palabras de ese idioma en el mismo archivo (el botón de la carpeta de firmware es `firmware.open_folder`). Todo el fabricante: quite su nombre de `firmware.vendor_unsupported`. Algunos chips: déjelo, ya que los demás chips del fabricante siguen mostrando ese texto
12. `app/src/ui/src/lib/links.ts`: la página de firmware del fabricante (en `LINKS`, `firmwareUrl()` y `vendorFirmwareUrl()`, como la carpeta de linux-firmware de Intel), para que la pantalla **Adaptador** muestre de dónde obtener los archivos; pruebas en `app/src/ui/src/lib/chips.test.ts`. La pantalla **Adaptador** muestra la página solo para los adaptadores cuya nota no sea `firmware.vendor_unsupported`, así que los demás chips del fabricante no la reciben

**Documentación**:

13. La tabla de chips de `docs/setup.md` y `docs/*/setup.md` (todos los idiomas): una fila como la de Intel, con los chips que cubre, los archivos de firmware y dónde van (la carpeta de firmware: `%APPDATA%\BlueCaster\firmware`, o su subcarpeta `<vendor>`; las filas más antiguas todavía dicen "config folder") y *Experimental.* Nombre los botones de la ventana como los tiene el `app/lang/*.json` de cada idioma (`ui.nav.adapter` para la pantalla Adaptador, `firmware.open_folder` para el botón que abre esa carpeta). Todo el fabricante: quítelo de la fila "Not supported". Algunos chips: deje ahí el fabricante y nombre los chips que siguen sin ser compatibles. Añada el fabricante a la frase al principio de la sección Firmware ("Intel and Broadcom adapters (experimental) may need firmware files of their own")
14. [Hardware verificado](hardware) (`docs/hardware.md`) cuando alguien lo haya usado, y la introducción de esta página (`docs/chip-support.md`, todos los idiomas), que nombra los fabricantes sin cargador (algunos chips: "the other <vendor> chips")

## Paso 7: Probarlo en el adaptador

Compile en la raíz del repositorio (`cargo build --release -p bluecaster-core-exe`), cierre la aplicación instalada (desde su icono de la bandeja) y ejecute el core desde la línea de comandos con una captura HCI:

```bat
target\release\bluecaster-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

El registro muestra cada paso; `loader.pklg` se abre en Wireshark con cada comando, evento y transferencia de control en orden. Desconecte el adaptador (o apague el equipo) y vuelva a probar: la descarga tiene que funcionar con el chip en frío y no hacer nada cuando el firmware ya está en ejecución.

{: .note }
Con varios adaptadores en WinUSB, se prefiere uno Realtek y se aplica su configuración de chip, incluso con `-u`. Ponga **Adaptador > Chip Bluetooth** en **No Realtek**, o pruebe solo con su adaptador en WinUSB.

Después, transmita un rato a unos auriculares desde la ventana y compruebe que **Reiniciar Bluetooth** y una reconexión del adaptador lo recuperan. Mientras se inicia el adaptador, la pantalla **Adaptador** muestra el cargador debajo del estado de Bluetooth ("cargando 42%", o que falló); en el modo de desarrollador también cuando está listo, con sus nombres de `step()` y cómo terminó.

## Paso 8: Abrir el pull request

Antes de abrirlo, ejecute lo siguiente en este orden (consulte [Compilación](building) para el entorno). Primero la interfaz, en `app\src\ui`: comprueba el cambio del paso 6 (punto 12), y el workspace de Rust incluye el crate de la ventana, que necesita la interfaz compilada:

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

Después, en la raíz del repositorio:

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

Sin Node.js, ejecute en su lugar las pruebas y clippy en los tres crates de arriba (`-p bt-host -p bluecaster-core -p bluecaster-core-exe`), deje las comprobaciones de la interfaz a la CI del pull request e indíquelo en el pull request. `pwsh ./build.ps1 -Test` lo ejecuta todo.

Clippy también comprueba los nombres: una variante de `State` llamada `PatchState` no pasa `enum_variant_names`; llámela `Patch`. `check_lang.py` puede informar de "long hints left" que ya estaban antes de su cambio; solo los errores y las claves que faltan le corresponde corregirlos a usted.

Otros lugares mencionan de pasada MediaTek o Qualcomm como no compatibles (los datos simulados y las pruebas de la interfaz, comentarios en `Adapter.tsx` y `probe.rs`, la opción «No Realtek»): se quedan como están.

En el pull request:

- Basado en la rama `develop-v*` actual
- Indique qué adaptadores probó (producto, VID:PID, chip), en frío y en caliente, y adjunte un informe de sondeo y la captura HCI de una descarga
- Indique de dónde salen los datos del cargador
- El cargador sigue siendo **experimental** hasta que lo hayan usado algunas personas; la pantalla **Adaptador** muestra esa etiqueta
