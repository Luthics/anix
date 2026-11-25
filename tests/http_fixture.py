"""Loopback HTTP request reader shared by the Responses adapter tests."""
import base64
from http.server import BaseHTTPRequestHandler


class JsonRequest(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def body(self):
        if self.headers.get("Transfer-Encoding", "").lower() != "chunked":
            return self.rfile.read(int(self.headers["Content-Length"]))
        chunks = []
        while True:
            size = int(self.rfile.readline().split(b";", 1)[0].strip(), 16)
            if not size:
                self.rfile.readline()
                return b"".join(chunks)
            chunks.append(self.rfile.read(size))
            if self.rfile.read(2) != b"\r\n":
                raise ValueError("invalid chunk terminator")


def pictures(node):
    """Read actual image blocks, retaining wire order, MIME and decoded bytes."""
    if isinstance(node, list):
        for item in node:
            yield from pictures(item)
    elif isinstance(node, dict):
        kind = node.get("type")
        if kind in ("image_url", "input_image"):
            url = node["image_url"]
            if isinstance(url, dict):
                url = url["url"]
            if not url.startswith("data:"):
                raise ValueError("expected an inline image data URI")
            media_type, data = url[5:].split(";base64,")
            yield media_type, base64.b64decode(data, validate=True)
        elif kind == "image":
            source = node["source"]
            if source["type"] != "base64":
                raise ValueError("expected a base64 image source")
            yield source["media_type"], base64.b64decode(source["data"], validate=True)
        else:
            for item in node.values():
                yield from pictures(item)
