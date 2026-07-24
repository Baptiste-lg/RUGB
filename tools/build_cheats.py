#!/usr/bin/env python3
"""
Convert libretro-database .cht files into a single cheats.json for RUGB.

Usage:
    # Clone the database first:
    git clone --depth 1 https://github.com/libretro/libretro-database.git /tmp/libretro-db

    # Then run:
    python3 tools/build_cheats.py /tmp/libretro-db/cht web/cheats.json
"""

import json
import os
import re
import sys


def parse_cht_file(path):
    """Parse a single .cht file into a list of cheat entries."""
    cheats = []
    current = {}

    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue

            m = re.match(r"cheat(\d+)_desc\s*=\s*\"(.+?)\"", line)
            if m:
                idx = int(m.group(1))
                if current and "desc" in current:
                    cheats.append(current)
                current = {"desc": m.group(2)}
                continue

            m = re.match(r"cheat(\d+)_code\s*=\s*\"(.+?)\"", line)
            if m:
                current["code"] = m.group(2)
                continue

    if current and "desc" in current and "code" in current:
        cheats.append(current)

    return cheats


def normalize_title(filename):
    """Extract game title from filename (strip extension and region tags)."""
    name = os.path.splitext(filename)[0]
    # Remove region tags like (USA), (Europe), (Japan), etc.
    name = re.sub(r"\s*\(.*?\)", "", name)
    # Remove revision tags like [!], [S], etc.
    name = re.sub(r"\s*\[.*?\]", "", name)
    return name.strip()


def scan_directory(cht_dir, system_name):
    """Scan a system's .cht directory and return a dict of title -> cheats."""
    db = {}
    if not os.path.isdir(cht_dir):
        return db

    for fname in sorted(os.listdir(cht_dir)):
        if not fname.endswith(".cht"):
            continue
        path = os.path.join(cht_dir, fname)
        cheats = parse_cht_file(path)
        if not cheats:
            continue

        title = normalize_title(fname)
        # Use uppercase title as key for case-insensitive matching
        key = title.upper()

        if key in db:
            # Merge cheats from different region variants
            existing_descs = {c["desc"] for c in db[key]["cheats"]}
            for c in cheats:
                if c["desc"] not in existing_descs:
                    db[key]["cheats"].append(c)
        else:
            db[key] = {"title": title, "system": system_name, "cheats": cheats}

    return db


def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <libretro-db/cht> <output.json>")
        sys.exit(1)

    cht_root = sys.argv[1]
    output_path = sys.argv[2]

    all_cheats = {}

    systems = [
        ("Nintendo - Game Boy", "gb"),
        ("Nintendo - Game Boy Color", "gbc"),
        ("Nintendo - Game Boy Advance", "gba"),
    ]

    total_games = 0
    total_cheats = 0

    for dir_name, sys_name in systems:
        cht_dir = os.path.join(cht_root, dir_name)
        db = scan_directory(cht_dir, sys_name)
        for key, entry in db.items():
            all_cheats[key] = entry
            total_games += 1
            total_cheats += len(entry["cheats"])

    # Write compact JSON
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(all_cheats, f, separators=(",", ":"), ensure_ascii=False)

    print(f"Built {output_path}: {total_games} games, {total_cheats} cheats")


if __name__ == "__main__":
    main()
