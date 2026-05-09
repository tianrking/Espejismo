# Client Profiles

`espejismo-local` can export and import compact client profiles.

Export from an existing TOML config:

```bash
espejismo-local --config espejismo.toml --print-client-profile --profile-name laptop
```

The output is an `espejismo://import/...` URL containing URL-safe base64 JSON.
It includes the PSK and optional local proxy credentials, so treat the profile
URL as secret key material. Base64 is only an encoding, not encryption.

Import:

```bash
espejismo-local --import-profile 'espejismo://import/...' --socks5-listen 127.0.0.1:1080
```

Profiles currently carry the local client essentials: profile name, remote
server address, PSK, local proxy listeners, and optional local proxy auth. Server
egress/admin/logging policy remains in TOML because those are deployment-side
operator settings.
