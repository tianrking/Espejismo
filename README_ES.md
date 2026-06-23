# Espejismo

Espejismo es un tunel cifrado nativo en Rust para redes publicas o no
confiables. El binario `espejismo-remote` corre en el servidor remoto y
`espejismo-local` expone SOCKS5, proxy HTTP y TUN opcional en el cliente.

## Instalacion desde Releases

Linux/macOS o Windows Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

Variables utiles:

```bash
ESPEJISMO_VERSION=latest
ESPEJISMO_PACKAGE=full
ESPEJISMO_INSTALL_DIR=$HOME/.espejismo
ESPEJISMO_REPO=tianrking/Espejismo
```

El instalador solo descarga y extrae el paquete de GitHub Releases. No crea
servicios ni reglas de firewall.

## Configuracion

Use `configs/examples/espejismo.toml` como unico archivo de configuracion para
cliente y servidor. El servidor usa `[shared]`, `[remote]`, `[logging]` y
`[admin]`; el cliente usa `[shared]`, `[local]`, `[logging]` y `[admin]`.

Edicion minima:

```toml
[shared]
psk = "cambie-esto-por-un-secreto-largo"

[local]
server = "IP_O_DOMINIO_DEL_SERVIDOR:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"
```

Servidor:

```bash
~/.espejismo/bin/espejismo-remote --config ~/.espejismo/configs/espejismo.toml
```

Cliente:

```bash
~/.espejismo/bin/espejismo-local --config ~/.espejismo/configs/espejismo.toml
```

Documentacion completa de parametros:
[`docs/deployment/CONFIG.md`](docs/deployment/CONFIG.md).
