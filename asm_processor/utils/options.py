from dataclasses import dataclass

@dataclass
class Opts:
    opt: str
    framepointer: bool
    mips1: bool
    kpic: bool
    pascal: bool
    input_enc: str
    output_enc: str
    enable_cutscene_data_float_encoding: bool
