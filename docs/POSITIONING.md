# Espejismo 定位与特色

> 本文档定义项目的产品定位。任何优化、重构、新功能都不得改变这里定义的定位；
> 如有冲突，先停下来讨论。技术上可以参考优秀开源项目（见
> `docs/research/REFERENCES.md`），但学的是手段，不是定位。

## 一句话定位

**Espejismo 是一条原生 Rust 加密隧道：不伪装任何协议，靠认证加密混沌对抗流量分析，
用最小的运维模型（一个服务端二进制、一个客户端二进制、一个 TOML 配置）跑通私人流量出海。**

English: Espejismo is a native Rust encrypted tunnel. Instead of impersonating
TLS/HTTP/QUIC, it presents authenticated encrypted chaos; instead of a complex
multi-protocol suite, it ships one server binary, one client binary, one TOML
config.

## 设计哲学：拒绝伪装

主流做法是"伪装派"：把流量打扮成 TLS、HTTP/2、QUIC，借用大厂协议的指纹做掩护。
Espejismo 走相反的路：

- **不借用任何协议的指纹**：没有 TLS 握手、没有证书故事、没有 ALPN、没有 HTTP
  头、不借用 QUIC。被动观察者看不到稳定的明文 TLV 标记、固定握手偏移、固定帧长元数据。
- **认证加密混沌**：X25519 建会、动态 HKDF 握手窗口、XOR 掩码的变长信封、
  XChaCha20-Poly1305 帧、可选填充。看到的只是不可解析的随机字节。
- **这不是隐身声明**：文档原话 — "This is not a claim of invisibility. It is a
  refusal to depend on camouflage."（不承诺隐身，只拒绝依赖伪装。）

## 核心特色

1. **极简运维模型**：`espejismo-remote`（服务端）+ `espejismo-local`（客户端）+
   一个 TOML。release 一键安装脚本，不建服务、不写防火墙规则、不藏后台进程。
2. **Rust 原生全平台**：Linux / macOS / Windows，客户端支持 SOCKS5、HTTP 代理、
   原生 TUN 接管（IPv4 路由/DNS）。
3. **传输多样性**：TCP/yamux 多 lane 池、WebSocket underlay、HTTP/2 underlay、
   确定性端口跳变。自适应吞吐 profile 应对跨洋高 RTT。
4. **抗探测工程**：客户端解谜（anti-replay 成本）、握手重放摘要缓存、动态握手
   窗口（录制的首包过期即失效）、静默拒绝、公网侧资源硬上限。
5. **可验证**：协议有完整 SPEC（`docs/PROTOCOL.md`），关键路径有可执行测试，
   benchmark 数据公开在 `docs/testing/`。

## 我们不做什么

- 不做协议伪装（no TLS/QUIC camouflage），不跟伪装派拼指纹逼真度。
- 不做大而全的多协议套件（不像 sing-box 那样收录几十种协议）。
- 不做中心化服务、不做账号体系、不碰用户流量内容。

## 与主流方案对比

| 方案 | 路线 | 配置复杂度 | Espejismo 的差异 |
| --- | --- | --- | --- |
| Xray / v2ray | 协议伪装派（VLESS+TLS+uTLS） | 高（多入站/出站/路由规则） | 我们不伪装；单配置文件 |
| Hysteria2 | QUIC 暴力拥塞控制，主打速度 | 中 | 我们走 TCP/yamux 稳定路线；不依赖 UDP |
| Shadowsocks | 轻量 AEAD 流 | 低 | 我们有完整握手认证、抗重放、端口跳变 |
| WireGuard | L3 VPN | 低 | 我们是应用层隧道，代理语义（SOCKS5/HTTP/TUN），更贴近出海场景 |

## 适用场景

- 个人/小团队的出海加密隧道，VPS 一键部署。
- 对"零伪装、少特征"有偏好的用户：不希望流量长得像某大厂服务。
- 运维能力有限、想要"装上就能用、配置一眼看懂"的用户。

## 版本状态

当前 `v0.1.5`。持续优化中（性能、稳定性、工程质量），定位不变。
