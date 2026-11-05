---
title: Integración (IPC)
layout: default
parent: Inicio
nav_order: 2.7
---

# Integración (IPC)
{: .no_toc }

Cómo se comunica una herramienta de terceros con el núcleo de BlueCaster: mostrar el estado de la transmisión, iniciar y detener perfiles, ajustar el volumen o acceder a los servicios propios de los auriculares a través del enlace Bluetooth de BlueCaster. Las herramientas que lo hacen aparecen en [Funciona con](works-with).

{: .warning }
> El protocolo aún puede cambiar durante las betas de 2.0.2. La fuente de referencia es [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/app/src/crates/ipc/src/protocol.rs) y los tipos de TypeScript generados a partir de él en [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/src/ui/src/ipc/bindings). Esta página explica cómo usarlos; no enumera todos los campos.

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

## Conexión {#connecting}

El núcleo (`bluecaster-core.exe`) se ejecuta mientras BlueCaster está en ejecución, con o sin su ventana. Escucha en una canalización con nombre del usuario con el que se ejecuta:

```
\\.\pipe\bluecaster-<user SID>
```

- `<user SID>` es la cadena del SID del usuario que ha iniciado sesión (`S-1-5-21-...`), obtenida por ejemplo con `whoami /user`. Cada usuario tiene su propio núcleo y su propia canalización; la DACL de la canalización solo admite a ese usuario, y los clientes remotos se rechazan.
- La canalización está en modo de bytes. Pueden conectarse varios clientes a la vez (la ventana de BlueCaster es uno de ellos); todos reciben el mismo estado y los mismos eventos.
- Hasta la 2.0.x, la canalización se llamaba `\\.\pipe\a2dpwb-<user SID>`. BlueCaster 3.0.0 atiende **ambos** nombres y sigue atendiendo el antiguo durante al menos 6 meses después de la 3.0.0. Un cliente debe probar primero `bluecaster-<user SID>` y, si no está, el nombre antiguo. La versión del protocolo sigue siendo 1.
- Si la canalización no existe, el núcleo no está en ejecución. Para comprobarlo, use la canalización, no el mutex del núcleo.
- Las compilaciones de desarrollo iniciadas con `BLUECASTER_INSTANCE=<name>` escuchan en cambio en `\\.\pipe\bluecaster-<SID>-<name>`, para que puedan ejecutarse junto a la instalada.

### Tramas {#frames}

Cada mensaje, en ambos sentidos, es una trama: la longitud del JSON como **u32 little-endian** y después el JSON (UTF-8). Las tramas de más de 4 MiB se rechazan.

### Negociación inicial {#handshake}

1. El cliente envía `hello` en los 5 segundos siguientes a conectarse:
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` es un nombre y una versión libres; aparece en el registro del núcleo.
2. El núcleo responde con `welcome`, o con `reject` y cierra la canalización (versión de protocolo incorrecta, o sin hello):
   ```json
   {"type": "welcome", "protocol": 1, "server": "bluecaster-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. El núcleo envía un `snapshot` con todo su estado y, después, `event`s.

La versión del protocolo debe coincidir exactamente. Las ampliaciones que mantienen funcionando la versión 1 se anuncian en cambio en `features` (consulte [Compatibilidad](#compatibility)).

## Estado y eventos {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

El cliente no guarda ningún estado propio: conserva la última instantánea y le aplica cada evento. Cada evento de estado **reemplaza una parte completa** de la instantánea:

| `kind` del evento | Reemplaza |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`, `le_scanning` | `scanning`, `le_scanning` |
| `stream` | `stream` (estado, mensaje, perfil, información del códec, volumen, batería) |
| `profiles` | `profiles` y `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`, `verbose`, `crashes` | el campo de ese nombre |

`stream.state` es uno de `idle`, `connecting`, `streaming`, `reconnecting`, `error`; `stream.message` indica el motivo, como una [clave de mensaje](#errors-and-messages).

**Revisiones.** Cada evento de estado lleva la revisión siguiente (`rev` + 1). Un salto significa que se perdió un evento: envíe `state.snapshot` para obtener una instantánea nueva. Un cliente que lee demasiado despacio también recibe del núcleo un `snapshot` nuevo sin pedirlo. Los eventos que no son de estado (`link`, `log`, `receiver`, `rfcomm_data`, `rfcomm_closed`) llevan la revisión actual y no la hacen avanzar.

Cuando el núcleo se cierra, envía el evento `bye` y cierra la canalización.

## Solicitudes {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` es cualquier número que elija el cliente; la respuesta lo devuelve. Las solicitudes se ejecutan de forma concurrente, así que las respuestas pueden llegar en otro orden que las solicitudes.
- `params` se omite en los métodos sin parámetros.
- Una solicitud que cambia el estado responde cuando ha terminado, y el cambio llega también como evento. Los clientes deben mostrar el estado a partir de los eventos, no de la respuesta.

### Errores y mensajes {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` es para el programa (`adapter.driver_missing`, `app.streaming`, ...). `message` es una clave de traducción de los archivos de idioma de BlueCaster ([`app/lang`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/lang)) con argumentos al estilo de printf, igual que los textos de estado del estado; una herramienta puede buscar ahí la clave o mostrar su propio texto para el código.

### Métodos {#methods}

Métodos para herramientas:

| Método | Parámetros | Resultado |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`; antes detiene una transmisión en curso |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume` (volumen absoluto AVRCP 0-127) | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds` (3-600) | `Device[]`, también como evento `devices` |
| `device.scan_stop` | -- | `null`; responde el `device.scan` en curso |
| `audio.devices` | -- | `AudioDevice[]` (orígenes de captura) |
| `diag.subscribe` | `logs` (nivel o `null`), `link` (bool), `receiver` (bool, opcional) | `null`; inicia o detiene los eventos `log` / `link` / `receiver` para esta conexión |
| `diag.info` | `{"mask": bool}` (opcional) | `DiagInfo` (el texto de «copiar información de diagnóstico») |
| `rfcomm.open`, `rfcomm.send`, `rfcomm.close` | consulte [Paso directo RFCOMM](#rfcomm-passthrough) | |

Métodos que usa la ventana de BlueCaster y que una herramienta solo debe usar con cuidado: `profile.save` / `profile.delete`, `settings.set` (reemplaza **toda** la configuración: envíe el `settings` más reciente con su cambio, nunca un objeto parcial), `device.forget` / `device.rename`, `adapter.*` (`adapter.assign_winusb` y `adapter.restore_driver` muestran una solicitud de UAC; `adapter.probe` y `adapter.run_script`, las herramientas de chip del modo de desarrollador, detienen la pila Bluetooth), `app.quit`. Los demás (`stream.start_custom`, el resto de `device.*` (`device.import_windows_pairings` muestra una solicitud de UAC), `audio.apps`, `audio.mixer` / `audio.mixer_set`, los `diag.*` aparte de los dos anteriores, `receiver.*`, `app.*`) sirven a las pantallas propias de la ventana y pueden cambiar sin previo aviso. La lista completa con parámetros y resultados es el enum `Method` de `protocol.rs`.

## Paso directo RFCOMM {#rfcomm-passthrough}

Disponible cuando `welcome.features` contiene `"rfcomm"` (2.0.2-beta.10 y posteriores). Permite a una herramienta acceder a los servicios RFCOMM propios de los auriculares (apps de control del fabricante: batería, control de ruido, EQ) a través del enlace que BlueCaster ya tiene, ya que Windows no ve los auriculares conectados mediante BlueCaster.

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`. `address` debe ser el dispositivo al que se está transmitiendo; el núcleo nunca conecta ni empareja para esto (`rfcomm.not_connected`). `uuid` es un UUID completo de 128 bits o un `"xxxx"` de 16 bits sobre el UUID base de Bluetooth. La respuesta llega en 20 s, también cuando está en cola detrás de otras aperturas: espere al menos 25 s. Errores: `rfcomm.not_connected`, `rfcomm.service_not_found`, `rfcomm.open_failed`, `rfcomm.busy`.
- `rfcomm.send {channel_id, data}` → `null`. `data` va en base64 (estándar, con relleno), como máximo 64 KiB por solicitud, y se envía en tramas de como máximo `mtu`, en el orden de las solicitudes. Errores: `rfcomm.no_channel`, `rfcomm.busy` (cola llena, no se envió), `rfcomm.timeout` (no se sabe si se envió: no lo repita, vuelva a abrir el canal).
- Los datos recibidos llegan como eventos `rfcomm_data` `{channel_id, data}`, solo a la conexión que abrió el canal.
- `rfcomm.close {channel_id}` → `null`, después de los datos enviados antes; no le sigue ningún `rfcomm_closed`.
- Un canal también se cierra cuando termina la conexión con la canalización, o con un evento `rfcomm_closed` `{channel_id, reason}`: `remote` (el dispositivo lo cerró o el enlace se cayó), `disconnected` (terminó la conexión A2DP), `stopped` (se detuvo la pila Bluetooth), `overflow` (el cliente dejó sin leer más de 1 MiB de datos recibidos).

## Compatibilidad {#compatibility}

- **Funciones.** Compruebe `welcome.features` antes de usar una ampliación. Los núcleos anteriores a 2.0.2-beta.10 omiten el campo.
- **Solicitudes desconocidas.** Una solicitud que el núcleo no puede leer (un método desconocido, parámetros incorrectos) recibe como respuesta el error `ipc.bad_request`; la conexión sigue abierta. Los núcleos anteriores a 2.0.2-beta.10 cierran en cambio la conexión. Cualquier otra trama ilegible la cierra.
- **Campos nuevos.** El núcleo añade campos al estado y a los eventos con el tiempo. Ignore los campos que no conozca; no rechace un mensaje por ellos.
- **Cambio de nombre en 3.0.0** ([#193](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/193)): la canalización pasa a ser `\\.\pipe\bluecaster-<SID>`. El núcleo sigue escuchando también en `\\.\pipe\a2dpwb-<SID>`, con el mismo protocolo, durante al menos 6 meses después de 3.0.0; su retirada se anuncia antes en las notas de la versión. Pruebe primero `bluecaster-<SID>` y, si no está, `a2dpwb-<SID>`. El mutex del núcleo cambia de nombre sin esa transición.

## Ejemplo {#example}

Un cliente mínimo en Python (solo con la biblioteca estándar) que muestra el estado de la transmisión y sigue sus cambios:

```python
import json, struct, subprocess

sid = subprocess.run(["whoami", "/user", "/fo", "csv", "/nh"],
                     capture_output=True, text=True).stdout.strip().split(",")[1].strip('"')
pipe = open(rf"\\.\pipe\bluecaster-{sid}", "r+b", buffering=0)

def send(msg):
    b = json.dumps(msg).encode()
    pipe.write(struct.pack("<I", len(b)) + b)

def read_exact(n):
    buf = b""
    while len(buf) < n:
        chunk = pipe.read(n - len(buf))
        if not chunk:
            raise EOFError
        buf += chunk
    return buf

def recv():
    (n,) = struct.unpack("<I", read_exact(4))
    return json.loads(read_exact(n))

send({"type": "hello", "protocol": 1, "client": "example/0.1"})
welcome = recv()
assert welcome["type"] == "welcome", welcome
snap = recv()
print("stream:", snap["state"]["stream"]["state"])
while True:
    m = recv()
    if m["type"] == "event" and m["event"]["kind"] == "stream":
        s = m["event"]["value"]
        print("stream:", s["state"], (s.get("info") or {}).get("codec"))
    elif m["type"] == "event" and m["event"]["kind"] == "bye":
        break
```

Para probar una herramienta sin tocar el BlueCaster instalado, ejecute una compilación de desarrollo con `BLUECASTER_INSTANCE` y `BLUECASTER_CONFIG_DIR` definidas (consulte [Compilación](building)) y conéctese a su canalización.
