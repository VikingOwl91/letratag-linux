import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import itertools
import json
from pathlib import Path
import struct

OUT = Path('/tmp/opencode/letratag-geometry-offline/ipp-boundary')
OUT.mkdir(exist_ok=True)
COUNTER = itertools.count(1)


def parse(data):
    pos = 8
    attrs = []
    while pos < len(data):
        tag = data[pos]
        pos += 1
        if tag == 3:
            return attrs, pos
        if tag < 16:
            attrs.append({'group': tag})
            continue
        size = struct.unpack_from('>H', data, pos)[0]
        pos += 2
        name = data[pos:pos + size].decode('utf8', errors='replace')
        pos += size
        size = struct.unpack_from('>H', data, pos)[0]
        pos += 2
        value = data[pos:pos + size]
        pos += size
        if tag in (0x21, 0x23) and size == 4:
            value = struct.unpack('>i', value)[0]
        elif tag == 0x22 and size == 1:
            value = bool(value[0])
        elif tag >= 0x40:
            value = value.decode('utf8', errors='replace')
        else:
            value = value.hex()
        attrs.append({'tag': tag, 'name': name, 'value': value})
    raise ValueError('missing end of attributes')


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def do_POST(self):
        if self.headers.get('Transfer-Encoding', '').lower() == 'chunked':
            body = bytearray()
            while True:
                size = int(self.rfile.readline().split(b';')[0], 16)
                if not size:
                    while self.rfile.readline() != b'\r\n':
                        pass
                    break
                body.extend(self.rfile.read(size))
                assert self.rfile.read(2) == b'\r\n'
            body = bytes(body)
        else:
            size = int(self.headers['Content-Length'])
            assert size < 16 * 1024 * 1024
            body = self.rfile.read(size)
        index = next(COUNTER)
        stem = OUT / f'{index:03d}'
        stem.with_suffix('.ipp').write_bytes(body)
        attrs, end = parse(body)
        document = body[end:]
        if document:
            stem.with_suffix('.document').write_bytes(document)
        stem.with_suffix('.json').write_text(json.dumps({
            'operation': struct.unpack_from('>H', body, 2)[0],
            'path': self.path,
            'headers': dict(self.headers),
            'attributes': attrs,
            'document_offset': end,
            'document_length': len(document),
            'document_sha256': hashlib.sha256(document).hexdigest(),
        }, indent=2))
        connection = http.client.HTTPConnection('127.0.0.1', 38671, timeout=60)
        connection.request('POST', self.path, body, {'Content-Type': 'application/ipp'})
        response = connection.getresponse()
        result = response.read()
        self.send_response(response.status)
        self.send_header('Content-Type', response.getheader('Content-Type', 'application/ipp'))
        self.send_header('Content-Length', str(len(result)))
        self.end_headers()
        self.wfile.write(result)
        connection.close()


ThreadingHTTPServer(('127.0.0.1', 38672), Handler).serve_forever()
