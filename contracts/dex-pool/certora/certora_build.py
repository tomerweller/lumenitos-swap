#!/usr/bin/env python3
"""
Certora Sunbeam build script for dex-pool contract.
"""

import argparse
import json
import subprocess
import tempfile
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parent  # contracts/dex-pool
WORKSPACE_ROOT = PROJECT_DIR.parent.parent  # workspace root

# Build command
COMMAND = f"cargo build --release --target wasm32-unknown-unknown -p dex-pool --features certora"

# Source files
SOURCES = ["src/**/*.rs", "Cargo.toml"]

# Output WASM binary
EXECUTABLES = str(WORKSPACE_ROOT / "target/wasm32-unknown-unknown/release/dex_pool.wasm")

VERBOSE = False

def log(msg):
    if VERBOSE:
        print(msg, file=sys.stderr)

def run_command(command, to_stdout=False):
    """Runs the build command and dumps output to temporary files."""
    log(f"Running '{command}'")
    try:
        if to_stdout:
            result = subprocess.run(
                command,
                shell=True,
                text=True,
                cwd=str(WORKSPACE_ROOT)
            )
            return None, None, result.returncode
        else:
            with tempfile.NamedTemporaryFile(delete=False, mode='w', prefix="certora_build_", suffix='.stdout') as stdout_file, \
                tempfile.NamedTemporaryFile(delete=False, mode='w', prefix="certora_build_", suffix='.stderr') as stderr_file:
                result = subprocess.run(
                    command,
                    shell=True,
                    stdout=stdout_file,
                    stderr=stderr_file,
                    text=True,
                    cwd=str(WORKSPACE_ROOT)
                )
                return stdout_file.name, stderr_file.name, result.returncode
    except Exception as e:
        log(f"Error running command '{command}': {e}")
        return None, None, -1

def write_output(output_data, output_file=None):
    """Writes the JSON output either to a file or dumps it to the console."""
    if output_file:
        with open(output_file, 'w') as f:
            json.dump(output_data, f, indent=4)
        log(f"Output written to {output_file}")
    else:
        print(json.dumps(output_data, indent=4), file=sys.stdout)

def main():
    parser = argparse.ArgumentParser(description="Compile dex-pool contract for Certora Prover.")
    parser.add_argument("-o", "--output", metavar="FILE", help="Path to output JSON to a file.")
    parser.add_argument("--json", action="store_true", help="Dump JSON output to the console.")
    parser.add_argument("-l", "--log", action="store_true", help="Show log outputs from cargo build on standard out.")
    parser.add_argument("-v", "--verbose", action="store_true", help="Be verbose.")

    args = parser.parse_args()
    global VERBOSE
    VERBOSE = args.verbose

    to_stdout = args.log

    # Compile and dump logs
    stdout_log, stderr_log, return_code = run_command(COMMAND, to_stdout)

    if stdout_log is not None:
        log(f"Temporary log file located at:\n\t{stdout_log}\nand\n\t{stderr_log}")

    # JSON output
    output_data = {
        "project_directory": str(PROJECT_DIR),
        "sources": SOURCES,
        "executables": EXECUTABLES,
        "success": True if return_code == 0 else False,
        "return_code": return_code,
        "log": {"stdout": stdout_log, "stderr": stderr_log}
    }

    if args.output:
        write_output(output_data, args.output)

    if args.json:
        write_output(output_data)

    sys.exit(0 if return_code == 0 else 1)

if __name__ == "__main__":
    main()
