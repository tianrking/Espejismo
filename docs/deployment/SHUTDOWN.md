# Shutdown and Connection Draining

Espejismo handles Ctrl-C/SIGINT and, on Unix, SIGTERM in both service
binaries. Receiving either signal stops the service loop and lets the process
exit. This is a prompt process shutdown, not a coordinated drain of active
tunnels: the binaries do not expose a readiness transition or transfer active
sessions to a replacement process. The remote does impose a bounded peer
handler drain period.
Active connections can be interrupted when the process exits. The local
service also aborts its listener tasks on a shutdown signal. The remote stops
its accept loop, then gives active peer handlers up to 10 seconds to finish
before aborting remaining work.

## Signals handled by the service binaries

- Ctrl-C is handled through Tokio's Ctrl-C listener on all supported platforms
  (reported as SIGINT on Unix). On Unix, SIGTERM is handled as well. The first
  of these events ends the service loop; repeated signals do not add a drain
  period.
- On Windows, the service code listens for Ctrl-C; it does not register a
  separate SIGTERM handler.
- The remote binary treats SIGHUP as a request to reload its original config
  source on Unix. A failed reload leaves current settings active and the
  service running. The local binary does not handle SIGHUP. Neither binary
  reopens its log file on SIGHUP. SIGUSR1 and SIGUSR2 retain their operating
  system default behavior. Configuration changes that support runtime
  application can also use the authenticated admin API; see [Admin API](ADMIN.md).
- SIGKILL and SIGSTOP cannot be handled by an application. Other signals are
  not converted into shutdown requests by these handlers.
- The local process aborts and joins its listener tasks after a handled
  shutdown event. The remote process stops accepting connections and waits up
  to 10 seconds for peer handlers to finish. Neither path migrates a live
  tunnel to the replacement process; unfinished work can be interrupted.

## Production deployments

A restart is a stop followed by a fresh process start; it does not transfer
sessions or proxy work to the replacement process. On the local side, the
shutdown handler aborts its listener tasks, so local SOCKS5, HTTP, and TUN
traffic can fail while it is stopped. On the remote side, active peer handlers
have up to 10 seconds to complete before remaining work is interrupted. Once
the replacement is healthy, clients can establish new
connections, but applications must retry interrupted requests or transfers.

- Treat a restart or stop as a brief outage for clients using that process.
  Clients may reconnect, but an in-flight stream is not resumed transparently.
- For a rolling deployment behind an external load balancer, first withdraw the
  remote instance from new traffic and allow the load balancer's own connection
  policy to run. Then stop or replace the process. Removing an instance from
  new traffic does not make Espejismo wait for existing connections.
- For a single remote, schedule a maintenance window and notify users before
  restarting. Upgrade the remote and local independently, confirming service
  health after each restart.
- The supplied systemd units use the normal SIGTERM stop signal. A
  `TimeoutStopSec` override can bound how long systemd waits before it force
  kills a process, but it does not enable connection draining. Choose it for
  the deployment's stop deadline, not as a promise that tunnels will complete.
- Keep TUN cleanup guidance in [TUN mode](TUN.md) in mind if the local process
  is stopped abnormally.

Typical systemd stop/restart commands are:

```bash
sudo systemctl stop espejismo-remote
sudo systemctl start espejismo-remote
```

## Native mux idle drain is separate

`shared.mux.native_drain_timeout_secs` configures the native mux's GOAWAY drain
window when a mux session becomes idle. Existing streams can finish while new
streams are rejected during that session drain; the session closes when the
streams finish or the timeout expires. This setting applies only to the native
mux's idle-session lifecycle. It does not control SIGTERM shutdown, and it does
not change the default Yamux mode. See [Configuration](CONFIG.md#sharedmux).

The binaries currently have no configurable process shutdown grace period.
Increasing the native mux drain timeout or systemd stop timeout does not
provide one.
