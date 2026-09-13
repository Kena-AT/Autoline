#!/usr/bin/env python3
"""Simple benchmark using criterion.py"""
import subprocess
import json
import sys

# Run cargo bench and capture output
result = subprocess.run(
    ["cargo", "bench", "-p", "autoline-core"],
    capture_output=True,
    text=True,
    cwd=r"C:\Users\hp\Documents\Programming\Projects\Rust Projects\Autoline"
)

# Look for benchmark results in the output
output = result.stdout + result.stderr

# criterion usually outputs a JSON summary or text summary
# Let's check what's there
lines = output.split('\n')
for line in lines:
    if 'Ben' in line or ' criterion' in line or 'Time' in line:
        print(line)

if not any('Ben' in l or ' criterion' in l for l in lines):
    print("No benchmark lines found, showing last 20 lines:")
    for line in lines[-20:]:
        print(line)