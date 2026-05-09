#!/usr/bin/env python3
import argparse
import socket
import struct


def recv_exact(sock: socket.socket, size: int) -> bytes:
    data = b""
    while len(data) < size:
        chunk = sock.recv(size - len(data))
        if not chunk:
            raise RuntimeError("unexpected EOF")
        data += chunk
    return data


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--socks-host", default="127.0.0.1")
    parser.add_argument("--socks-port", type=int, required=True)
    parser.add_argument("--username", required=True)
    parser.add_argument("--password", required=True)
    parser.add_argument("--target-host", default="127.0.0.1")
    parser.add_argument("--target-port", type=int, required=True)
    parser.add_argument("--payload", required=True)
    args = parser.parse_args()

    tcp = socket.create_connection((args.socks_host, args.socks_port), timeout=10)
    tcp.sendall(b"\x05\x01\x02")
    if recv_exact(tcp, 2) != b"\x05\x02":
        raise RuntimeError("SOCKS server did not select username/password auth")
    user = args.username.encode()
    password = args.password.encode()
    tcp.sendall(b"\x01" + bytes([len(user)]) + user + bytes([len(password)]) + password)
    if recv_exact(tcp, 2) != b"\x01\x00":
        raise RuntimeError("SOCKS username/password auth failed")

    tcp.sendall(b"\x05\x03\x00\x01\x00\x00\x00\x00\x00\x00")
    reply = recv_exact(tcp, 4)
    if reply[:2] != b"\x05\x00":
        raise RuntimeError(f"UDP ASSOCIATE failed: {reply!r}")
    atyp = reply[3]
    if atyp == 1:
        host = socket.inet_ntop(socket.AF_INET, recv_exact(tcp, 4))
    elif atyp == 4:
        host = socket.inet_ntop(socket.AF_INET6, recv_exact(tcp, 16))
    else:
        raise RuntimeError(f"unsupported bind atyp {atyp}")
    port = struct.unpack("!H", recv_exact(tcp, 2))[0]

    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp.settimeout(10)
    target = socket.inet_aton(args.target_host)
    payload = args.payload.encode()
    packet = b"\x00\x00\x00\x01" + target + struct.pack("!H", args.target_port) + payload
    udp.sendto(packet, (host, port))
    response, _ = udp.recvfrom(65535)
    header_len = 4 + 4 + 2
    body = response[header_len:]
    expected = b"udp-echo:" + payload
    if body != expected:
        raise RuntimeError(f"unexpected UDP response: {body!r}")
    print(body.decode())


if __name__ == "__main__":
    main()
