import encoder_py_bindings

import numpy as np
import sys
# insert at 1, 0 is the script path (or '' in REPL)
sys.path.insert(1, '/Users/jon/Dev/Cacophony/python-cptv/cptv')

import time

from datetime import datetime
from typing import Iterable
from reader import CPTVReader
from writer import CPTVWriter
from frame import Frame

filename = sys.argv[1]

def write_cptv(frames: Iterable[Frame], filename: str):
    with open(filename, "wb") as f:
        w = CPTVWriter(f)
        w.timestamp = datetime.now()
        w.device_name = b"foo42"
        w.latitude = 142.2
        w.longitude = -39.2
        w.loc_timestamp = datetime.now()
        w.accuracy = 20
        w.altitude = 200
        w.preview_secs = 3
        w.motion_config = b"stuff"
        w.fps = 30
        w.model = b"ultra"
        w.brand = b"laser"
        w.firmware = b"killer"
        w.camera_serial = 221

        w.write_header()

        tic = time.perf_counter()
        for frame in frames:
            w.write_frame(frame)
        toc = time.perf_counter()
        fps = len(frames) / (toc - tic)
        print(f"Wrote {len(frames)} frames in {toc - tic:0.4f} seconds ({fps:0.4f}fps)")
        w.close()

with open(filename, "rb") as f:
    reader = CPTVReader(f)
    t0 = None

    frames = []
    for frame in reader:
        frames.append(frame)
    write_cptv(frames, "out.cptv")

#print()
