"""Opt-in TCP/UDP echo responder for controlled Tropa routing tests."""

import argparse
import ipaddress
import re
import socket
import socketserver

REQUEST = re.compile(rb"TROPA1 ([0-9a-fA-F]{16,128}:[0-9]{1,5})\n")


def reply_for(data, source):
    match = REQUEST.fullmatch(data)
    if match is None:
        return None
    address = str(ipaddress.ip_address(source))
    return b"TROPA1 " + match[1] + b" " + address.encode("ascii") + b"\n"


class UdpHandler(socketserver.BaseRequestHandler):
    def handle(self):
        data, connection = self.request
        reply = reply_for(data, self.client_address[0])
        if reply is not None:
            connection.sendto(reply, self.client_address)


class TcpHandler(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.settimeout(2)
        data = bytearray()
        try:
            while len(data) <= 256 and not data.endswith(b"\n"):
                chunk = self.request.recv(257 - len(data))
                if not chunk:
                    return
                data.extend(chunk)
            reply = reply_for(bytes(data), self.client_address[0])
            if reply is not None:
                self.request.sendall(reply)
        except (TimeoutError, OSError):
            return


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True, help="Literal IP to bind on the controlled server")
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--transport", choices=("tcp", "udp"), required=True)
    args = parser.parse_args()
    address = ipaddress.ip_address(args.host)
    if address.is_multicast or not 1024 <= args.port <= 65535:
        parser.error("Use a unicast address and an unprivileged port")
    base = socketserver.TCPServer if args.transport == "tcp" else socketserver.UDPServer

    class Server(base):
        address_family = socket.AF_INET6 if address.version == 6 else socket.AF_INET
        allow_reuse_address = True

    handler = TcpHandler if args.transport == "tcp" else UdpHandler
    with Server((str(address), args.port), handler) as server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
