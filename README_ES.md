# Espejismo

**[🇬🇧 English](README.md) &nbsp;|&nbsp; [🇪🇸 Español](README_ES.md)**

<p>
  <img src="https://img.shields.io/badge/Rust-1.75%2B-dea584?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Tokio-async_runtime-ff5e00?logo=tokio&logoColor=white" alt="Tokio">
  <img src="https://img.shields.io/badge/XChaCha20--Poly1305-AEAD-4a90d9" alt="AEAD">
  <img src="https://img.shields.io/badge/X25519-key__exchange-e97326" alt="X25519">
  <img src="https://img.shields.io/badge/License-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey" alt="Platform">
</p>

Un tunel de transporte cifrado multiplataforma nativo en Rust para redes publicas
y no confiables. Ingreso local via SOCKS5 y HTTP, secreto directo con X25519,
frames cifrados con XChaCha20-Poly1305, multiplexacion yamux, padding adaptativo
y puzzles de cliente — todo en Rust seguro sin TUN/TAP ni dependencias del sistema.

## Arquitectura

### Vista del Sistema

```mermaid
flowchart LR
    subgraph Local["espejismo-local"]
        APP["Aplicacion"]
        SOCKS["Ingreso SOCKS5<br/>TCP CONNECT + UDP ASSOCIATE"]
        HTTP["Ingreso proxy HTTP<br/>CONNECT + HTTP absolute-form"]
        AUTH["Auth local opcional"]
        YMUX_C["Sesion cliente yamux<br/>streams logicos"]
        ENC_C["Adaptador de transporte cifrado"]
    end

    subgraph Wire["Conexion TCP publica"]
        FLOW["Flujo de bytes protegido con AEAD<br/>perfil estandar o stealth"]
    end

    subgraph Remote["espejismo-remote"]
        PROBE["Guardia de probes<br/>fallback HTTP o tarpit silencioso"]
        HS["Verificador de handshake<br/>HMAC + X25519 + puzzle + cache replay"]
        ENC_R["Adaptador de transporte cifrado"]
        YMUX_R["Sesion servidor yamux"]
        REQ["Parser de solicitudes de tunel"]
        POLICY["Politica de salida<br/>ACL de host + puerto"]
        DEST["Destino TCP / UDP"]
    end

    APP --> SOCKS --> AUTH --> YMUX_C
    APP --> HTTP --> AUTH
    YMUX_C --> ENC_C --> FLOW --> PROBE --> HS --> ENC_R --> YMUX_R
    YMUX_R --> REQ --> POLICY --> DEST
```

El detalle importante de la estructura en capas es que yamux gestiona los
streams logicos, mientras `spawn_frame_transport` proporciona a yamux un
objeto `AsyncRead + AsyncWrite` normal respaldado por frames cifrados. Las
solicitudes del proxy local se convierten en streams yamux; el socket fisico
transporta solo el transporte cifrado.

### Pila de Protocolo

```text
Trafico de aplicacion
  -> parser de proxy SOCKS5 / HTTP
  -> auth de proxy local opcional
  -> stream logico yamux
  -> transporte de frames cifrados
  -> socket TCP
  -> handshake remoto / defensas replay / probe
  -> manejador de streams yamux
  -> politica de salida
  -> conexion TCP o rele UDP de un solo disparo
```

### Handshake

El modo estandar comienza con un hello de cliente autenticado de longitud
variable:

```text
[ HMAC-SHA256 32 ][ timestamp UTC 8 ][ nonce 24 ][ clave publica X25519 32 ]
[ version de protocolo 2 ][ capacidades 8 ][ nonce puzzle 8 ]
[ longitud padding 2 ][ padding 0..N ]
```

El cliente resuelve un puzzle acotado de SHA-256 con ceros iniciales sobre el
cuerpo antes de calcular el HMAC. El remoto verifica el puzzle, comprueba la
desviacion de timestamp, valida el HMAC en tiempo constante, y registra la
clave publica efimera en una cache de replay acotada. Las claves de sesion se
derivan con X25519 y HKDF-SHA256.

Cuando `profile = "stealth"`, el intercambio hello se envuelve en dos bloques
de tamano fijo que coinciden con `shared.stealth.frame_size`. La carga util
del bloque esta enmascarada con un flujo XOR derivado de HMAC y padding
aleatorio, por lo que el handshake no expone la longitud del hello en modo
plain ni el hello del servidor de tamano fijo.

### Transporte de Frames

Los perfiles estandar usan frames AEAD con longitud prefijada enmascarada:

```text
[ longitud cifrada enmascarada 4 ][ XChaCha20-Poly1305(tipo || payload) ]
```

`low_latency`, `balanced`, y `high_entropy` ajustan la aleatorizacion de
fragmentos, jitter, y padding adaptativo alrededor de ese formato de frame
estandar.

El modo stealth usa frames AEAD de tamano fijo sin cabecera de longitud:

```text
[ XChaCha20-Poly1305 texto cifrado exactamente shared.stealth.frame_size bytes ]

texto plano antes del cifrado:
[ tipo 1 ][ payload_len 2 ][ payload ][ padding aleatorio hasta tamano fijo ]
```

La bomba de subida envia un calentamiento corto de padding aleatorio tras el
handshake stealth, luego escribe frames de datos o padding en un calendario
dosificado. Si no hay datos de aplicacion en cola, la cadencia de inactividad
decae desde el `tick_ms` base hacia intervalos mas lentos tipo heartbeat; los
datos reales reinician la cadencia. Se aplica un pequeno jitter pre-escritura
para que los frames de datos y padding no tengan un comportamiento de
planificador identico.

### Comportamiento de Probe y Fallback

Los pares desconocidos o invalidos no reciben error de protocolo. Dependiendo
de la configuracion remota, son retenidos en un tarpit silencioso acotado o,
para probes con aspecto HTTP, enrutados a un upstream de fallback configurado.
Si no hay upstream configurado, el fallback integrado devuelve una pequena
respuesta HTTP 200 con cabeceras `Date`, `Last-Modified`, `ETag`,
`Content-Length`, `Connection`, y `Server` dinamicas. Un upstream real como
Nginx/Caddy sigue siendo el fallback de produccion preferido porque hereda una
huella de servidor web completa y natural.

### Lo que Stealth Ayuda a Mitigar

| Senal observable | Mitigacion en este codigo | Salvedad restante |
| --- | --- | --- |
| Tamano del handshake en plain | Stealth envuelve hello/respuesta en bloques enmascarados de tamano fijo | Los primeros dos bloques siguen siendo metadatos de inicio de conexion |
| Distribucion de tamano de frames | Todos los frames de datos, cierre, y padding stealth usan un tamano | Los flujos de tamano fijo pueden ser inusuales por si mismos |
| Comportamiento de rafaga/silencio | Los frames de padding continuan cuando no hay datos de aplicacion en cola | La cadencia de inactividad decae deliberadamente para reducir huellas de flujo constante |
| Asimetria de direccion | Ambos lados ejecutan el mismo comportamiento de transporte stealth | La planificacion del kernel y la congestion pueden diferir por direccion |
| Clasificacion de payload | AEAD oculta el tipo y contenido del frame | El volumen de trafico, endpoint, y duracion siguen siendo visibles |
| Probes HTTP activos | Fallback a upstream opcional o respuesta integrada dinamica | El fallback integrado es una comodidad, no un sustituto de un sitio web real |

Stealth es un perfil de conformacion de trafico, no una garantia matematica de
indetectabilidad. Reduce varias huellas de protocolo obvias, pero los
observadores de red pueden seguir modelando metadatos como la reputacion del
endpoint, duracion de la conexion, volumen total de bytes, comportamiento de
reintento, y efectos de congestion.

## Plataformas Soportadas

| Plataforma | Arquitectura | Estado |
| --- | --- | --- |
| Linux | amd64, 386, arm64, armv7 | Soportado |
| macOS | Apple Silicon (arm64) | Soportado |
| Windows | amd64, 386, arm64 | Soportado |

## Compilacion

```bash
cargo build --release
```

CI multiplataforma verifica Linux, macOS y Windows. El workflow de release genera
artefactos empaquetados para:

- `linux-amd64`
- `linux-386`
- `linux-arm64`
- `linux-armv7`
- `darwin-arm64`
- `windows-amd64`
- `windows-386`
- `windows-arm64`

Cada archivo contiene:

- `bin/espejismo-local`
- `bin/espejismo-remote`
- `configs/espejismo.toml`
- README y notas de arquitectura/testing

Crear paquete para el host Unix-like actual:

```bash
./scripts/package-release.sh
```

Crear paquete en Windows PowerShell:

```powershell
.\scripts\package-release.ps1
```

Tambien se puede pasar un target triple de Rust instalado:

```bash
rustup target add x86_64-unknown-linux-gnu
./scripts/package-release.sh x86_64-unknown-linux-gnu
```

## Inicio Rapido

### Linux/macOS

Terminal 1 — servidor remoto:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-remote -- --listen 0.0.0.0:6690
```

Terminal 2 — cliente local:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-local -- \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681 \
  --server 127.0.0.1:6690
```

### Windows PowerShell

Terminal 1 — servidor remoto:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
cargo run --bin espejismo-remote -- --listen 127.0.0.1:6690
```

Terminal 2 — cliente local:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
cargo run --bin espejismo-local -- --socks5-listen 127.0.0.1:6680 --http-listen 127.0.0.1:6681 --server 127.0.0.1:6690
```

Luego apunta un cliente SOCKS5 a `127.0.0.1:6680` o un cliente de proxy HTTP
a `127.0.0.1:6681`.

### Instalacion Remota en Ubuntu con Un Comando

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=latest bash
```

Ver [docs/deployment/QUICKSTART.md](docs/deployment/QUICKSTART.md) para todas
las variables del instalador y la configuracion del cliente Windows.

## Configuracion

Generar una configuracion TOML inicial:

```bash
cargo run --bin espejismo-local -- --print-example-config > espejismo.toml
```

La misma configuracion sirve para ambos binarios; cada uno lee su seccion correspondiente.

```toml
[shared]
psk = "change-me-long-random-secret"
clock_skew_secs = 30
puzzle_bits = 12
max_padding = 64
jitter_ms = 0
padding_chance_percent = 35
backpressure_threshold_ms = 40
backpressure_cooldown_ms = 1000
tunnel_buffer = 1048576
idle_timeout_secs = 300
max_streams = 256

[shared.obfuscation]
profile = "balanced"
randomize_chunks = true
min_chunk = 1024
max_chunk = 16384

[shared.stealth]
frame_size = 4096
tick_ms = 50

[local]
server = "127.0.0.1:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"
handshake_padding = 256

[local.auth]
username = "local-user"
password = "local-pass"

[logging]
level = "info"
format = "compact"
ansi = true
# file = "/var/log/espejismo/espejismo.log"

[admin]
# listen = "127.0.0.1:9090"
# token = "change-me-admin-token"

[remote]
listen = "0.0.0.0:6690"
handshake_timeout_ms = 3000
reject_delay_ms = 0
max_handshake_padding = 1024
replay_window_secs = 60
cold_start_delay_ms = 35
tarpit_max = 1024
tarpit_hold_secs = 300

[remote.egress]
deny_private_ips = false
allow_hosts = []
block_hosts = []
allow_ports = []
block_ports = []
```

Ejecutar desde un archivo:

```bash
cargo run --bin espejismo-remote -- --config espejismo.toml
cargo run --bin espejismo-local -- --config espejismo.toml
```

Ejecutar desde un release empaquetado:

```bash
./bin/espejismo-remote --config configs/espejismo.toml
./bin/espejismo-local --config configs/espejismo.toml
```

Release empaquetado en Windows:

```powershell
.\bin\espejismo-remote.exe --config .\configs\espejismo.toml
.\bin\espejismo-local.exe --config .\configs\espejismo.toml
```

Asistente de configuracion en Windows:

```powershell
.\scripts\setup-windows.ps1 -Mode local -Server "IP_DEL_SERVIDOR:6690" -Psk "el-mismo-psk"
```

Ejecutar desde TOML codificado en base64, util para paneles de despliegue o
importaciones de un solo comando:

```bash
CONFIG_B64="$(base64 -w0 espejismo.toml)"
cargo run --bin espejismo-remote -- --config-base64 "$CONFIG_B64"
cargo run --bin espejismo-local -- --config-base64 "$CONFIG_B64"
```

Imprimir un ejemplo directamente en base64:

```bash
cargo run --bin espejismo-local -- --print-example-config-base64
```

## Handshake

El protocolo de handshake se describe en detalle en la seccion
[Arquitectura](#arquitectura) anterior, cubriendo los modos plain y stealth.
Los detalles internos adicionales del protocolo y la especificacion del formato
en vivo se encuentran en [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Notas

- `espejismo-local --socks5-listen` habilita el proxy SOCKS5 local.
- `espejismo-local --http-listen` habilita el proxy HTTP local.
- `[local.auth]` habilita autenticacion SOCKS5 por usuario/contrasena y
  autenticacion HTTP Basic del proxy. Omitir para un listener sin autenticacion
  solo en loopback confiable.
- `[logging]` controla los logs estructurados. `format` puede ser `compact`,
  `pretty`, o `json`; `file` escribe logs a un archivo en lugar de stderr.
- `--log-level`, `--log-format`, `--log-file`, y `--no-log-ansi` sobreescriben
  la configuracion de logging para ambos binarios.
- `[admin]` habilita un endpoint HTTP admin de solo lectura con `/healthz`,
  `/status`, y `/metrics`. Usar `token` fuera de entornos loopback confiables.
- `[remote.egress]` controla la politica de salida del servidor con listas de
  hosts y puertos permitidos/bloqueados.
- `espejismo-local --print-client-profile` emite un URL de perfil
  `espejismo://import/...` que puede importarse con `--import-profile`.
- SOCKS5 soporta `CONNECT` TCP y `ASSOCIATE` UDP. Los datagramas UDP se retransmiten
  por streams yamux autenticados y son verificados por la politica de salida remota.
- `--max-padding` controla el tamano maximo del payload de los frames de padding
  cifrados.
- `--padding-chance-percent` controla la frecuencia con la que se intenta el padding.
- `--backpressure-threshold-ms` detecta escrituras lentas y deshabilita el padding.
- `--backpressure-cooldown-ms` controla cuanto tiempo permanece deshabilitado el
  padding tras una escritura lenta.
- `--jitter-ms` aplica un pequeno retraso aleatorio antes de enviar frames.
- `[shared.obfuscation]` controla la forma del trafico del emisor. `profile` puede
  ser `low_latency`, `balanced`, `high_entropy`, o `stealth`; `randomize_chunks`
  y los limites de fragmentos varian los tamanios de frames cifrados antes de
  agregar padding.
- `[shared.stealth]` se usa cuando `profile = "stealth"`: cada frame cifrado
  mide exactamente `frame_size` bytes. El transporte comienza con un
  calentamiento corto de padding aleatorio, envia datos o padding en una
  cadencia dosificada, y gradualmente reduce el padding de inactividad hacia
  intervalos tipo heartbeat antes de que los datos reales lo reinicien.
- `--puzzle-bits` configura la dificultad del puzzle del cliente. Valores limitados
  a 24 bits.
- `espejismo-local --handshake-padding` controla el padding aleatorio maximo en
  el primer paquete.
- `espejismo-remote --max-handshake-padding` limita el padding aceptado en el
  primer paquete.
- `espejismo-remote --replay-window-secs` controla la ventana de la cache de
  replay en memoria.
- `espejismo-remote --handshake-timeout-ms` acota los handshakes incompletos.
- `espejismo-remote --reject-delay-ms` agrega un retraso de cierre silencioso
  acotado para handshakes invalidos. Valores superiores a 10000 ms son limitados.
- `espejismo-remote --tarpit-max` controla el tamano del tarpit silencioso acotado
  usado cuando `reject_delay_ms = 0`.
- `espejismo-remote --tarpit-hold-secs` controla cuanto tiempo se retienen los
  sockets invalidos en el tarpit silencioso acotado.
- `[remote.fallback_http]` controla el comportamiento ante probes activos. Usar
  `mode = "silent"` para manejo silencioso acotado, o `mode = "http_fallback"`
  para enrutar prefijos de probe HTTP a un endpoint TCP `upstream` configurado
  (por ejemplo nginx local) o a una pagina 200 OK interna.
- `--tunnel-buffer` controla el buffer de transporte cifrado en proceso usado
  por debajo de yamux.
- `espejismo-remote --cold-start-delay-ms` aplica un pequeno retraso de inicio
  tras un handshake valido y antes de que comience yamux.
- La PSK acepta `hex:...`, `base64:...`, o una cadena UTF-8 cruda.
- Los handshakes invalidos se cierran silenciosamente por defecto. Con
  `[remote.fallback_http].enabled = true`, los probes reciben respuestas de
  fallback con aspecto HTTP en su lugar.
- El tarpit es intencionalmente silencioso: retiene sockets brevemente y nunca
  envia bytes de goteo a pares desconocidos.

## Smoke Test

```bash
./scripts/e2e_smoke.sh
```

En Windows PowerShell:

```powershell
.\scripts\e2e_smoke.ps1
```

El script inicia un servidor HTTP local, `espejismo-remote`, y `espejismo-local`,
luego realiza verificaciones de SOCKS5 TCP, SOCKS5 UDP, proxy HTTP, HTTP CONNECT,
admin, metricas e importacion de perfil a traves del tunel yamux cifrado.

## Logging

Los logs de consola por defecto usan formato compacto legible:

```toml
[logging]
level = "info"
format = "compact"
ansi = true
```

Para ingesta en produccion, usar logs JSON:

```toml
[logging]
level = "info,espejismo_core=debug"
format = "json"
ansi = false
file = "/var/log/espejismo/espejismo.log"
```

El campo `level` acepta directivas normales de filtro de tracing, para que los
operadores puedan elevar un modulo manteniendo el resto en silencio.

## Estado del Proyecto

Ver [docs/development/STATUS.md](docs/development/STATUS.md) para la matriz de
funcionalidades implementadas y la hoja de ruta restante, incluyendo migracion
transparente, empaquetado WASM/navegador, recarga en tiempo de ejecucion, y
control de multiples perfiles mas rico.

Ver [docs/testing/TEST_PLAN.md](docs/testing/TEST_PLAN.md) para la estrategia
de pruebas ejecutable y [docs/research/DESIGN_PRINCIPLES.md](docs/research/DESIGN_PRINCIPLES.md)
para los principios de diseno del protocolo.

## Uso Responsable

Espejismo esta destinado para acceso cifrado a sistemas que usted posee o esta
explicitamente autorizado a administrar, como un laboratorio casero, servidor
privado, o entorno de pruebas interno. No es un servicio, red de anonimato, ni
herramienta de evasion de autorizacion.

La conformacion de trafico puede reducir algunas huellas de protocolo, pero no
hace invisible una conexion. Los operadores deben asumir que los endpoints,
timing, volumen de bytes, uptime, ruta de enrutamiento, y errores de despliegue
pueden seguir siendo observables. Usar upstreams de fallback reales, logging
conservador, PSKs fuertes, y politica de salida restrictiva en produccion.

Usted es responsable de cumplir con todas las leyes aplicables, politicas de red,
terminos de servicio, controles de exportacion, y limites de autorizacion en su
jurisdiccion y en cualquier red donde despliegue o use este software. No use
Espejismo para acceder a sistemas sin permiso, evadir controles de acceso legales,
o violar regulaciones locales. Este README es documentacion tecnica, no consejo
legal; consulte a un profesional cualificado si su despliegue tiene riesgo legal
o de cumplimiento.
