# Process exit codes

`espejismo-local`, `espejismo-remote`, and the optional
`espejismo-bench-http` use Rust's `Result`-returning `main` entry point. They do
not define a project-specific set of exit codes. The codes below describe the
current CLI and process behavior; error text is diagnostic output, not a stable
machine-readable interface.

| Outcome | Exit status | Notes |
| --- | ---: | --- |
| Successful one-shot command or normal return | `0` | Includes help/version output, config printing, and successful checks. |
| CLI syntax or argument validation error | `2` | `clap` prints the usage/error message before exiting. |
| Application error returned from `main` | `1` | Includes config loading/validation failures, failed probes, startup/bind errors, and failed one-shot operations. The error chain is printed to stderr. |
| Panic or abnormal termination | Nonzero; exact status can vary | This is not an application error code and should be diagnosed from stderr, logs, or the service manager. |
| Termination by signal | Platform-dependent | A shell may report `128 + signal number`; service managers can report the signal separately from an exit status. |

These are process-level outcomes, not proxy protocol statuses. SOCKS5 and HTTP
request replies are documented in [Error and status reference](ERRORS.md).

## Automation and service managers

For scripts, treat `0` as success and any nonzero status as failure. Do not
branch on the wording of stderr: messages can gain context or change as error
handling evolves. Use `--check-config` for a configuration gate and
`--probe-server` for a client-side connectivity and handshake check.

With the supplied systemd units, `Restart=on-failure` restarts a service after
nonzero exit or abnormal termination. Inspect `systemctl status` and the
journal to distinguish an application exit from a signal or other abnormal
termination. See [systemd deployment](SYSTEMD.md) and
[troubleshooting](TROUBLESHOOTING.md).
