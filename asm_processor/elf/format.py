import struct

class ElfFormat:
    def __init__(self, is_big_endian):
        self.is_big_endian = is_big_endian
        self.struct_char = ">" if is_big_endian else "<"

    def pack(self, fmt, *args):
        return struct.pack(self.struct_char + fmt, *args)

    def unpack(self, fmt, data):
        return struct.unpack(self.struct_char + fmt, data)
