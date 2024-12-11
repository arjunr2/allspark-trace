import struct
from pathlib import Path
import json

from argparse import ArgumentParser

def parse_args():
    parser = ArgumentParser(prog='process_tracefiles.py', 
        description='Process tracefiles into readable formats')
    parser.add_argument('-d', '--dir', type=Path, required=True, help='Directory containing tracefiles')
    parser.add_argument('-v', '--verbose', action='store_true', help='Print verbose output')
    return parser.parse_args()

def main():
    args = parse_args()
    # Each 8-byte entry is packed as:
    # <3 byte target pc> <3 byte source pc> <2 byte function index> 
    timestamp_struct = struct.Struct('<Q')
    brentry_struct = struct.Struct('<Q')

    field_splits = [3, 3, 2]
    field_split_idxs = [
        sum(field_splits[:i]) for i in range(len([0] + field_splits))
    ]

    with open(args.dir / 'trace.aspk', 'rb') as f:
        data = f.read()

    cf_table = {}
    num_entries = 0    

    timestamp, = timestamp_struct.unpack(data[:timestamp_struct.size])

    for entry, in brentry_struct.iter_unpack(data[timestamp_struct.size:]):
        eb = entry.to_bytes(8, byteorder='little')
        target_pc, source_pc, func_idx = [
            int.from_bytes(
                eb[field_split_idxs[i]:field_split_idxs[i+1]], byteorder='little'
            ) for i in range(len(field_splits))
        ]
        
        if args.verbose:
            print(f'{func_idx} {source_pc:06x} {target_pc:06x}')

        if func_idx not in cf_table:
            cf_table[func_idx] = {}
        if source_pc not in cf_table[func_idx]:
            cf_table[func_idx][source_pc] = {}
        if target_pc not in cf_table[func_idx][source_pc]:
            cf_table[func_idx][source_pc][target_pc] = 0
        cf_table[func_idx][source_pc][target_pc] += 1

        num_entries +=1

    if args.verbose:
        print(json.dumps(cf_table, indent=2))

    print(f"Timestamp: {timestamp // 1_000_000} s | {(timestamp % 1_000_000) // 1_000} ms")
    print(f"Total dynamic entries: {num_entries}")
    num_branch_sites = 0
    target_types = {k: 0 for k in ["uncond", "cond", "table"]}
    for src_pc_table in cf_table.values():
        num_branch_sites += len(src_pc_table)
        for target_pc_table in src_pc_table.values():
            num_targets = len(target_pc_table.values())
            if num_targets == 1:
                target_types["uncond"] += 1
            elif num_targets == 2:
                target_types["cond"] += 1
            elif num_targets > 2:
                target_types["table"] += 1
    print(f"Number of Branch Sites (static): {num_branch_sites}")
    print("Branch Site distribution (dynamic): {}".format(
        json.dumps(target_types, indent=2)
    ))

if __name__ == '__main__':
    main()
