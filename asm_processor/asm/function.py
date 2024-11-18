from collections import namedtuple

Function = namedtuple('Function', [
    'text_glabels',           # List of glabels in the text section
    'asm_conts',              # List of raw assembly contents
    'late_rodata_dummy_bytes', # Bytes of dummy late rodata values
    'jtbl_rodata_size',       # Size of jump table rodata
    'late_rodata_asm_conts',  # List of late rodata contents
    'fn_desc',                # Function descriptor (type, name)
    'data',                   # Raw data (including rodata)
