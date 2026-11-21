#!/usr/bin/env python3
"""Check real curl stdout-pipe cancellation against a synthetic local stream."""
import http.server
import json
import subprocess
import threading
import time

class Stream(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        try:
            for i in range(1000):
                self.wfile.write(f'data: {i}\n\n'.encode())
                self.wfile.flush()
                time.sleep(0.01)
        except (BrokenPipeError, ConnectionResetError):
            pass

server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Stream)
threading.Thread(target=server.serve_forever, daemon=True).start()
cmd = ['curl', '-sS', '-i', '-N', '--http1.1', '--connect-timeout', '10',
       '--speed-time', '60', '--speed-limit', '1', f'http://127.0.0.1:{server.server_port}/']
child = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
while True:
    line = child.stdout.readline()
    if line.startswith(b'data:'):
        break
    if not line:
        raise RuntimeError('stream ended before data')
start = time.monotonic()
child.stdout.close()
try:
    status = child.wait(timeout=3)
    print(json.dumps({'probe':'real_curl_closed_stdout', 'exited_within_3_seconds':True,
                      'status':status, 'elapsed_ms':round((time.monotonic()-start)*1000,3),
                      'stderr':child.stderr.read().decode().strip()}))
except subprocess.TimeoutExpired:
    child.kill()
    child.wait()
    print(json.dumps({'probe':'real_curl_closed_stdout', 'exited_within_3_seconds':False}))
finally:
    server.shutdown()
    server.server_close()
