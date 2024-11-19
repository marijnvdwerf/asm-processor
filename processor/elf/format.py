import struct
from typing import Tuple

class ElfFormat:
    def __init__(self, is_big_endian: bool) -> None:
        self.is_big_endian = is_big_endian
        self.struct_char = ">" if is_big_endian else "<"

    def pack(self, fmt: str, *args: int) -> bytes:
        return struct.pack(self.struct_char + fmt, *args)

    def unpack(self, fmt: str, data: bytes) -> Tuple[int, ...]:
        return struct.unpack(self.struct_char + fmt, data)
