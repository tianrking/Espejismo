#!/usr/bin/env python3
import argparse
import http.server
import json
import socketserver


class ProbeHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        if self.path == "/release/latest":
            self._reply_json(
                {
                    "tag_name": "v99.0.0",
                    "html_url": "https://example.test/espejismo/v99.0.0",
                }
            )
            return
        self._reply(b"")

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length) if length else b""
        self._reply(body)

    def log_message(self, fmt, *args):
        print(fmt % args, flush=True)

    def _reply(self, body):
        self._reply_json(
            {
                "method": self.command,
                "path": self.path,
                "probe": self.headers.get("x-espejismo-probe", ""),
                "body": body.decode("utf-8", errors="replace"),
            },
        )

    def _reply_json(self, value):
        payload = json.dumps(value, sort_keys=True).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.send_header("connection", "close")
        self.end_headers()
        self.wfile.write(payload)


class ReusableThreadingTCPServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    with ReusableThreadingTCPServer((args.host, args.port), ProbeHandler) as server:
        server.serve_forever()


if __name__ == "__main__":
    main()
