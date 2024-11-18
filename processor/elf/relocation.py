from ..utils.constants import SHT_REL

class Relocation:
    def __init__(self, fmt, data, sh_type):
        self.fmt = fmt
        self.sh_type = sh_type
        if sh_type == SHT_REL:
            self.r_offset, self.r_info = fmt.unpack('II', data)
        else:
            self.r_offset, self.r_info, self.r_addend = fmt.unpack('III', data)
        self.sym_index = self.r_info >> 8
        self.rel_type = self.r_info & 0xff

    def to_bin(self):
        self.r_info = (self.sym_index << 8) | self.rel_type
        if self.sh_type == SHT_REL:
            return self.fmt.pack('II', self.r_offset, self.r_info)
        else:
            return self.fmt.pack('III', self.r_offset, self.r_info, self.r_addend)
