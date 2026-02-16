#!/usr/bin/env python3
"""Tool for analyzing raw MTGO packet captures"""

import struct
import json
from pathlib import Path


def read_dump_file(path):
    """Read packets from dump file created by PacketDumper"""
    packets = []
    with open(path, 'rb') as f:
        while True:
            timestamp_bytes = f.read(8)
            if len(timestamp_bytes) < 8:
                break

            len_bytes = f.read(4)
            if len(len_bytes) < 4:
                break

            timestamp = struct.unpack('>q', timestamp_bytes)[0]
            length = struct.unpack('>I', len_bytes)[0]
            data = f.read(length)

            packets.append({
                'timestamp': timestamp,
                'length': length,
                'data': data.hex(),
            })

    return packets


def find_patterns(packets):
    """Analyze packets for common patterns"""
    analysis = {
        'total_packets': len(packets),
        'length_distribution': {},
        'common_prefixes': {},
        'encoding_hints': [],
    }

    for packet in packets:
        length = packet['length']
        analysis['length_distribution'][length] = \
            analysis['length_distribution'].get(length, 0) + 1

        if length >= 4:
            prefix = packet['data'][:8]  # First 4 bytes as hex
            analysis['common_prefixes'][prefix] = \
                analysis['common_prefixes'].get(prefix, 0) + 1

    return analysis


def main():
    import sys
    if len(sys.argv) < 2:
        print("Usage: analyze_capture.py <dump_file>")
        sys.exit(1)

    path = Path(sys.argv[1])
    packets = read_dump_file(path)
    analysis = find_patterns(packets)

    print(json.dumps(analysis, indent=2))


if __name__ == '__main__':
    main()
