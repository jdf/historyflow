#!/usr/bin/env python3
"""Dev server for historyflow.

Plain static file serving plus Cache-Control: no-store, so a browser
reload always gets the fresh build.
"""
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".map": "application/json",
    }

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, format, *args):
        pass  # keep serve.log quiet; errors still surface via stderr


def main():
    directory = sys.argv[1] if len(sys.argv) > 1 else "."
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 8790
    server = ThreadingHTTPServer(
        ("127.0.0.1", port), partial(Handler, directory=directory)
    )
    print(f"serving {directory} on http://localhost:{port}/", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
