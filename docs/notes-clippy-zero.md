# Clippy zero

## 方案与预期

`validate_admin_listener` 用 `Option::map_or(true, ...)` 表达“管理地址未设置时校验通过”，触发 clippy 的 `unnecessary_map_or`。改用 `Option::is_none_or` 保持相同的空值语义并消除告警；没有运行时行为或性能变化，预期收益是工作区在 `-D warnings` 下通过。

这次是通用 Rust 代码质量修正，不涉及传输或协议实现；参考清单中的传输项目与此处无直接关联，不借入其协议或架构。

## 改动和验证

- 将管理监听地址检查改写为 `admin.is_none_or(...)`。
- 扩展现有 `rejects_admin_listener_reusing_proxy_address` 单元测试，分别覆盖管理端口与 SOCKS5、HTTP 地址冲突、独立管理地址通过，以及未配置管理地址通过。
- `cargo test -p espejismo-client`：31 passed, 0 failed；覆盖客户端完整单元测试，其中目标回归用例核验 SOCKS5/HTTP 冲突拒绝、独立地址接受和未配置接受。
- `cargo clippy --all-targets -- -D warnings`：通过，工作区所有 target 无 clippy 警告。
- 结论：告警清零门禁通过；校验语义保持不变，无行为或性能变化。
