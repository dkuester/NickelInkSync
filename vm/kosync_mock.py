#!/usr/bin/env python3
"""A minimal KOSync server for the Nickel VM: accepts any user, keeps the
progress documents in a JSON file so a test can read and preset them.

    kosync_mock.py PORT STATE.json
"""

import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1])
STATE = sys.argv[2]


def load():
    try:
        with open(STATE) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return {}


def save(docs):
    with open(STATE, 'w') as f:
        json.dump(docs, f, indent=2, sort_keys=True)


class Handler(BaseHTTPRequestHandler):
    def reply(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == '/users/auth':
            self.reply(200, {'authorized': 'OK'})
        elif self.path.startswith('/syncs/progress/'):
            doc = load().get(self.path[len('/syncs/progress/'):])
            self.reply(200, doc or {})
        else:
            self.reply(404, {'message': 'not found'})

    def do_PUT(self):
        if self.path != '/syncs/progress':
            self.reply(404, {'message': 'not found'})
            return
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        body['timestamp'] = int(time.time())
        docs = load()
        docs[body['document']] = body
        save(docs)
        self.reply(200, {'document': body['document'], 'timestamp': body['timestamp']})

    def do_POST(self):
        # Registration: every user exists already.
        self.reply(201, {'username': 'vm'})


ThreadingHTTPServer(('0.0.0.0', PORT), Handler).serve_forever()
