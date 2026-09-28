# 优秀开源项目参考清单

> 目的：学技术手段，不学定位。Espejismo 的定位见 `../POSITIONING.md`
> （不伪装、小运维模型、认证加密混沌），任何借鉴不得改变定位。
> 每个优化任务立项前，先看这里有没有现成的思路和数据。

## 传输与拥塞控制

- **hysteria2** (`apernet/hysteria`) — QUIC 传输、暴力拥塞控制（BBR 变体）、
  UDP 高速场景的实测数据。可学：拥塞控制参数调优方法、弱网下的重传策略、
  它的 benchmark 方法论。注意：我们走 TCP/yamux 路线，不跟进 QUIC。
- **quic-go** (`quic-go/quic-go`) — QUIC 协议最成熟的 Go 实现。可学：
  pacing、ACK 处理、丢包检测的状态机设计（思想可迁移到我们的自适应写入器）。
- **kcp-go** (`xtaci/kcp-go`) — KCP 重传/前向纠错。可学：高丢包链路的
  补偿思路（评估，不一定引入）。

## 多路复用

- **yamux** (`hashicorp/yamux`) — 我们在用的多路复用库。可学：窗口管理、
  keepalive、心跳超时的最佳实践；跟踪上游更新。
- **smux** (`xtaci/smux`) — 轻量多路复用。可学：帧头压缩、低延迟场景的
  调度策略，对比 yamux 的取舍。

## 代理框架与架构

- **sing-box** (`SagerNet/sing-box`) — 架构干净的通用代理内核。可学：
  模块划分、配置 schema 设计、跨平台 TUN 实现（尤其 Windows/macOS）。
  不学：收录几十种协议的大杂烩路线。
- **Xray-core** (`XTLS/Xray-core`) — 传输层抽象（TCP/mKCP/WS/H2/gRPC/QUIC）。
  可学：underlay 抽象层的接口设计；我们已有 WS/H2 underlay，可对照查漏。
- **shadowsocks-rust** (`shadowsocks/shadowsocks-rust`) — 纯 Rust、轻量、
  代码可读性高。可学：Rust 异步 IO 的写法、AEAD 帧处理、UDP relay 实现。

## 隐蔽与抗探测（批判性参考）

- **utls** (`refractionPOINT/utls`, Go) — TLS 指纹伪造。我们**不**做伪装，
  但要懂伪装派在玩什么，才能论证"不伪装"路线的合理性。
- **trojan-g** / **naiveproxy** — 流量行为模仿思路。参考其"行为像正常流量"
  的 pacing/shaping 思想，用于我们的 stealth shaper（是整形，不是伪装）。

## 性能测试方法

- **iperf3**、**nuttcp** — 基准测试的黄金标准；我们的 bench 脚本应对齐其
  统计口径（多轮中位数、预热、conf interval）。
- 各项目的 `bench` / CI perf 回归做法：性能优化必须带前后对比数据进仓库，
  这是我们的硬性门禁（见持续优化简报模板）。

## 使用规则

1. 借鉴前先读源码和文档，写进 `docs/notes-<topic>.md`：学了什么、为什么适用、
   预期收益、做了什么取舍。
2. 不得引入与定位冲突的依赖或协议（如 TLS 伪装、QUIC 迁移需先讨论）。
3. 引用要注明出处（项目名 + 链接），尊重上游 license（MIT/Apache-2.0/GPL
   的传染性要分清，进 `Cargo.toml` 的依赖先检查 license）。
