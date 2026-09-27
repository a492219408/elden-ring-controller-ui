"""只读生成最小官方总览描述；输出 Rust 源码到标准输出，不复制完整游戏资源。"""
import argparse
import hashlib
import struct
from pathlib import Path

COMMON_HASH = '4f80a029d8d6bdb7c6c893f13704e44c592c670c5adfc8ba6b11c793fb5e0392'

def tags(data):
    pos = 0
    while pos < len(data):
        head = struct.unpack_from('<H', data, pos)[0]
        pos += 2
        size = head & 63
        if size == 63:
            size = struct.unpack_from('<I', data, pos)[0]
            pos += 4
        body = data[pos:pos + size]
        assert len(body) == size
        pos += size
        yield head >> 6, body
        if head >> 6 == 0:
            assert size == 0 and pos == len(data)
            return
    raise ValueError('missing end')

class Bits:
    def __init__(self, data, at):
        self.data, self.bit = data, at * 8

    def take(self, n, signed=False):
        result = 0
        for _ in range(n):
            result = result * 2 + ((self.data[self.bit // 8] >> (7 - self.bit % 8)) & 1)
            self.bit += 1
        return result - (1 << n) if signed and n and result & (1 << (n - 1)) else result

def pose(code, body):
    assert code in (26, 70) and body[0] & 6 == 6 and not body[0] & 1
    offset = 4 if code == 70 else 3
    cid = struct.unpack_from('<H', body, offset)[0]
    bits = Bits(body, offset + 2)
    scale = None
    if bits.take(1):
        n = bits.take(5)
        x, y = bits.take(n, True), bits.take(n, True)
        assert x == y
        scale = x
    assert bits.take(1) == 0
    n = bits.take(5)
    translate = bits.take(n, True), bits.take(n, True)
    color = None
    if body[0] & 8:
        bits.bit = (bits.bit + 7) // 8 * 8
        assert bits.take(1) == 1 and bits.take(1) == 1
        n = bits.take(4)
        color = [bits.take(n, True) for _ in range(8)]
    return cid, scale, translate, color

def image(body):
    cid, kind, fmt, width, height = struct.unpack_from('<5H', body)
    assert kind == 0 and fmt == 13
    length = body[10]
    name = body[11:11+length].decode('ascii')
    tail = body[11+length:]
    assert tail[0] == len(name) + 4 and tail[1:].decode('ascii') == name + '.tga'
    return cid, f'ImageSpec {{ name: b"{name}", size: [{width}, {height}] }}'

def generate(source):
    data = source.read_bytes()
    assert len(data) == 46401 and hashlib.sha256(data).hexdigest() == COMMON_HASH
    top = list(tags(data[22:]))
    sprites = {struct.unpack_from('<H', b)[0]: list(tags(b[4:])) for c, b in top if c == 39}
    images = dict(image(b) for c, b in top if c == 1009)
    print('// 由 tools/generate-overview-presets.py 从精确官方 1.17 样本生成。')
    print('// 单位为原生 twip 和 16.16 定点数；不是图集裁剪坐标或目测补偿。')
    for constant, root, line in [('DUALSENSE',168,167), ('DUALSHOCK4',170,169), ('XBOX_SERIES',164,161)]:
        root_places = [(c,b) for c,b in sprites[root] if c in (26,70)]
        line_places = [(c,b) for c,b in sprites[line] if c in (26,70)]
        assert len(root_places) == 20 and len(line_places) == 1
        big, scale, position, big_color = pose(*root_places[0])
        line_cid, line_scale, line_position, line_color = pose(*root_places[1])
        image_cid, image_scale, image_position, image_color = pose(*line_places[0])
        assert line_cid == line and line_scale is None and image_position == (0, 0)
        assert big_color is None and image_color is None
        print(f'pub(super) const {constant}: Preset = Preset {{')
        print(f'    big: {images[big]},')
        print(f'    lines: {images[image_cid]},')
        print(f'    big_scale: {scale}, big_position: {list(position)},')
        print(f'    line_scale: {image_scale}, line_position: {list(line_position)},')
        print(f'    line_color: Some({line_color}),' if line_color else '    line_color: None,')
        print('    labels: [')
        for c, b in root_places[2:]:
            cid, scale, position, color = pose(c, b)
            assert cid == 163 and scale is None and color is None
            print(f'        {list(position)},')
        print('    ],\n};')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    generate(parser.parse_args().source)
