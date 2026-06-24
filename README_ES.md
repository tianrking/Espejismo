# Espejismo

[English](README.md) | [Configuracion](docs/deployment/CONFIG.md) | [Modo TUN](docs/deployment/TUN.md) | [Protocolo](docs/PROTOCOL.md) | [Benchmarks HK2/RK](docs/testing/V0.1.3_HK2_RK_MODE_MATRIX.md)

![Release](https://img.shields.io/badge/release-v0.1.3-0b7285)
![Rust](https://img.shields.io/badge/rust-native-9a3412)
![Plataformas](https://img.shields.io/badge/platforms-linux%20%7C%20macOS%20%7C%20windows-1f6feb)
![Entrada](https://img.shields.io/badge/ingress-socks5%20%7C%20http%20%7C%20tun-2f9e44)
![Licencia](https://img.shields.io/badge/license-MIT-495057)

Espejismo es un tunel cifrado nativo en Rust para enviar trafico privado de un
cliente a traves de un servidor remoto autenticado. El modelo operativo es
pequeno: un binario de servidor, un binario local, un archivo TOML compartido y
paquetes de release instalables con un solo comando.

## Perfil Tecnico

| Capa | Que incluye `v0.1.3` |
| --- | --- |
| Entrada local | SOCKS5, proxy HTTP y captura TUN nativa con controles UDP configurables |
| Salida remota | Listener TCP autenticado con politica de egress configurable |
| Transporte | TCP/yamux, pool multi-lane, underlay WebSocket, underlay HTTP/2 y port hopping deterministico |
| Criptografia | Sesion X25519, ventanas HKDF dinamicas de handshake, cache de replay y tramas XChaCha20-Poly1305 |
| Rutas | Toma de rutas/DNS IPv4 TUN en Linux, macOS y Windows |
| Empaquetado | Archivos GitHub Release completos y solo-servidor |

El egress del servidor tambien puede encadenarse por un proxy upstream:

```toml
[remote.egress]
proxy = "socks5://user:pass@127.0.0.1:1080"
# proxy = "http://user:pass@127.0.0.1:8080"
# proxy = "https://user:pass@proxy.example.com:8443"
```

SOCKS4/SOCKS4a, SOCKS5, HTTP CONNECT y HTTPS CONNECT soportan encadenamiento
TCP. El encadenamiento UDP requiere SOCKS5.

`espejismo-remote` corre en el VPS o servidor. `espejismo-local` corre en la
maquina cliente y expone puertos SOCKS5/HTTP locales o una interfaz TUN nativa
para captura IPv4 a nivel de sistema.

En `v0.1.3`, el modo TUN envia los flujos TCP/UDP de escritorio por lanes
interactivas por defecto y bloquea UDP/443 localmente salvo que se configure lo
contrario, haciendo que los navegadores vuelvan de QUIC a HTTPS sobre TCP en
lugar de acumular timeouts UDP largos.

Para datos reales HK2 a RK por modo, incluyendo TCP, stealth, WebSocket,
HTTP/2 y port hopping, vea [matriz v0.1.3 HK2/RK](docs/testing/V0.1.3_HK2_RK_MODE_MATRIX.md).

## Instalacion Desde Release

Linux, macOS o Windows Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

Variables del instalador:

| Variable | Valor por defecto | Uso |
| --- | --- | --- |
| `ESPEJISMO_VERSION` | `latest` | Tag de release, por ejemplo `v0.1.3` |
| `ESPEJISMO_PACKAGE` | `full` | `full` para cliente+servidor, `server` solo remoto |
| `ESPEJISMO_INSTALL_DIR` | `$HOME/.espejismo` | Directorio de extraccion |
| `ESPEJISMO_REPO` | `tianrking/Espejismo` | Repositorio GitHub |
| `ESPEJISMO_ARCHIVE_URL` | vacio | URL directa del archivo |

El instalador solo descarga y extrae el paquete de GitHub Releases. No crea
servicios, reglas de firewall, cambios de rutas ni procesos ocultos.

## Un Archivo De Configuracion

Use [configs/examples/espejismo.toml](configs/examples/espejismo.toml) como la
forma unica de configuracion para ambos lados. El servidor lee `[shared]`,
`[remote]`, `[logging]` y `[admin]`. El cliente lee `[shared]`, `[local]`,
`[logging]` y `[admin]`.

Edicion minima:

```toml
[shared]
psk = "cambie-esto-por-un-secreto-largo"

[shared.handshake_window]
enabled = true
step_secs = 30
previous_windows = 1
future_windows = 0

[shared.obfuscation]
profile = "stealth"
chunk_policy = "stealth"
randomize_chunks = false

[shared.stealth]
frame_size = 4096
frame_size_candidates = [3328, 3584, 4096, 4608]
tick_ms = 20

[local]
server = "IP_O_DOMINIO_DEL_SERVIDOR:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 2
bulk_lanes = 2

[remote]
listen = "0.0.0.0:6690"
```

`shared.handshake_window` deriva la clave del primer paquete desde el PSK y una
ventana corta de tiempo, asi que los handshakes grabados expiran rapido. Los
frames `stealth` ocultan longitudes estables con bloques cifrados de tamano
fijo. El pool de tuneles reparte nuevos streams logicos entre lanes TCP
independientes para reducir el head-of-line blocking de una sola conexion.

Ejecute el lado remoto en el servidor:

```bash
~/.espejismo/bin/espejismo-remote --config ~/.espejismo/configs/espejismo.toml
```

Ejecute el lado local en el cliente:

```bash
~/.espejismo/bin/espejismo-local --config ~/.espejismo/configs/espejismo.toml
```

Luego configure las aplicaciones con:

```text
SOCKS5: 127.0.0.1:6680
HTTP:   127.0.0.1:6681
```

Para captura a nivel de sistema, inicie el cliente con TUN:

```bash
sudo ~/.espejismo/bin/espejismo-local \
  --config ~/.espejismo/configs/espejismo.toml \
  --tun-enabled \
  --tun-auto-route \
  --tun-auto-dns
```

En Windows, ejecute la terminal como Administrador. Los archivos oficiales de
release para Windows incluyen `bin/wintun.dll` junto a `espejismo-local.exe`.

## Documentacion Operativa

| Tema | Enlace |
| --- | --- |
| Referencia completa de configuracion | [docs/deployment/CONFIG.md](docs/deployment/CONFIG.md) |
| Despliegue rapido | [docs/deployment/QUICKSTART.md](docs/deployment/QUICKSTART.md) |
| Modo TUN nativo | [docs/deployment/TUN.md](docs/deployment/TUN.md) |
| Flags CLI | [docs/deployment/CLI.md](docs/deployment/CLI.md) |
| Paquetes y releases | [docs/deployment/PACKAGING.md](docs/deployment/PACKAGING.md) |
| Contrato del protocolo | [docs/PROTOCOL.md](docs/PROTOCOL.md) |

## Compilar Desde Codigo Fuente

```bash
cargo build --release
cargo test --workspace --all-targets
```

Controles principales de calidad:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

## Estructura Del Proyecto

```text
crates/espejismo-core     Protocolo, crypto, config, admin, mux y transporte
crates/espejismo-client   espejismo-local
crates/espejismo-server   espejismo-remote
configs/examples          Ejemplo TOML unico
docs/deployment           Documentacion de configuracion y operacion
scripts                   Instaladores delgados para descargar releases
```

## Uso Responsable

Use Espejismo solo en sistemas y redes que usted posee o administra con permiso
explicito. El traffic shaping puede reducir algunas huellas estables, pero no
vuelve invisibles los IPs de los endpoints, el timing, el uptime, el volumen de
trafico ni los errores de despliegue.
