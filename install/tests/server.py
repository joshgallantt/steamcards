"""A stand-in for steamcards' GitHub releases, for the install scripts' tests.

    python3 server.py ROOT PORT_FILE

Serves the releases in ROOT at http://127.0.0.1:PORT/releases, where PORT is a
free port, which it writes to PORT_FILE once it's listening. Under /releases
are the addresses the install scripts use on
https://github.com/joshgallantt/steamcards/releases:

    /latest               redirects to /tag/vX.Y.Z, where X.Y.Z is what
                          ROOT/latest says. If ROOT/latest is empty, it
                          redirects to the releases page instead, as GitHub
                          does for a repository with no releases. With no
                          ROOT/latest, it's a 404.
    /tag/vX.Y.Z           the release's page
    /download/vX.Y.Z/F    redirects to the file ROOT/vX.Y.Z/F, as GitHub
                          redirects to where it keeps the files
    (nothing)             the releases page

The tests change ROOT between scenarios, and the server reads it afresh for
every request. Python's standard library only, so it runs wherever Python 3
does.
"""

import http.server
import os
import re
import socketserver
import sys
import urllib.parse

NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")


class Releases(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    root = "."

    def do_GET(self):
        self.answer(with_body=True)

    def do_HEAD(self):
        self.answer(with_body=False)

    def answer(self, with_body):
        path = urllib.parse.urlsplit(self.path).path
        # Strict, so a doubled or trailing / is a 404, not quietly fixed.
        parts = path.split("/")[1:]
        if not all(NAME.match(part) for part in parts):
            self.send_error(404)
        elif parts == ["releases"]:
            self.page("steamcards releases", with_body)
        elif parts == ["releases", "latest"]:
            self.latest()
        elif len(parts) == 3 and parts[:2] == ["releases", "tag"] and self.release(parts[2]):
            self.page("steamcards " + parts[2], with_body)
        elif len(parts) == 4 and parts[:2] == ["releases", "download"] and self.file(parts[2], parts[3]):
            self.redirect("/files/%s/%s" % (parts[2], parts[3]))
        elif len(parts) == 3 and parts[0] == "files" and self.file(parts[1], parts[2]):
            self.send_file(self.file(parts[1], parts[2]), with_body)
        else:
            self.send_error(404)

    def latest(self):
        try:
            with open(os.path.join(self.root, "latest"), encoding="utf-8") as f:
                version = f.read().strip()
        except FileNotFoundError:
            self.send_error(404)
            return
        self.redirect("/releases/tag/v" + version if version else "/releases")

    def release(self, tag):
        return os.path.isdir(os.path.join(self.root, tag))

    def file(self, tag, name):
        path = os.path.join(self.root, tag, name)
        return path if os.path.isfile(path) else None

    def redirect(self, path):
        self.send_response(302)
        self.send_header("Location", "http://127.0.0.1:%d%s" % (self.server.server_port, path))
        self.send_header("Content-Length", "0")
        self.end_headers()

    def page(self, title, with_body):
        self.send_body(b"<!doctype html><title>" + title.encode() + b"</title>\n", "text/html; charset=utf-8", with_body)

    def send_file(self, path, with_body):
        with open(path, "rb") as f:
            self.send_body(f.read(), "application/octet-stream", with_body)

    def send_body(self, body, content_type, with_body):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if with_body:
            self.wfile.write(body)


class Server(http.server.ThreadingHTTPServer):
    def server_bind(self):
        # HTTPServer's own server_bind looks up this machine's name, which on
        # some machines (GitHub's macOS runners) takes long enough that the
        # tests give up waiting. Nothing here uses the name.
        socketserver.TCPServer.server_bind(self)
        self.server_name = "127.0.0.1"
        self.server_port = self.server_address[1]


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: server.py ROOT PORT_FILE")
    root, port_file = sys.argv[1], sys.argv[2]
    Releases.root = root
    server = Server(("127.0.0.1", 0), Releases)
    # Written whole, then renamed, so the tests never read half of it.
    with open(port_file + ".tmp", "w", encoding="utf-8") as f:
        f.write("%d\n" % server.server_port)
    os.replace(port_file + ".tmp", port_file)
    server.serve_forever()


if __name__ == "__main__":
    main()
