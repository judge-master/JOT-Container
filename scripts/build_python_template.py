"""Link a prebuilt CPython 3.14 into a filesystem-free WASI P1 template.

Run with the native Python built from the same source as the WASI library.
The source slot and metadata match experiments/python-wasm/pack_template.rs.
"""

import argparse
import marshal
from pathlib import Path
import re
import struct
import subprocess
import sys


SOURCE_CAPACITY = 1024 * 1024
SOURCE_BEGIN = b'# JOT_PYTHON_SOURCE_V1_BEGIN\nraise RuntimeError("unpatched template")\n'
SOURCE_END = b"JOT_PYTHON_SOURCE_V1_END"
METADATA_NAME = b"jot.python.source.v1"


def frozen_modules(source):
    """Keep the module selection used by the standalone WASI execution probes."""
    library = source / "Lib"
    modules = [(path.stem, path, False) for path in sorted(library.glob("*.py"))]
    for package in ("encodings", "collections", "re"):
        for path in sorted((library / package).glob("*.py")):
            is_package = path.stem == "__init__"
            name = package if is_package else f"{package}.{path.stem}"
            modules.append((name, path, is_package))
    return modules


def generate_main(source, output):
    arrays = []
    entries = []
    modules = frozen_modules(source)
    for index, (name, path, is_package) in enumerate(modules):
        code = compile(path.read_bytes(), f"<frozen {name}>", "exec")
        data = ",".join(map(str, marshal.dumps(code)))
        arrays.append(f"static const unsigned char m{index}[] = {{{data}}};")
        entries.append(
            f'{{"{name}", m{index}, sizeof(m{index}), {int(is_package)}}},'
        )

    payload = SOURCE_BEGIN.ljust(SOURCE_CAPACITY - 1, b" ") + b"\0" + SOURCE_END + b"\0"
    payload_c = ",".join(map(str, payload))
    program = (
        '#include <Python.h>\n'
        + "\n".join(arrays)
        + "\nstatic const struct _frozen modules[] = {\n"
        + "\n".join(entries)
        + "\n{NULL, NULL, 0, 0}};\n"
        + f"static const char submission[] = {{{payload_c}}};\n"
        + """
int main(void) {
    PyImport_FrozenModules = modules;
    PyPreConfig pre;
    PyPreConfig_InitIsolatedConfig(&pre);
    pre.utf8_mode = 1;
    PyStatus status = Py_PreInitialize(&pre);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }

    PyConfig config;
    PyConfig_InitIsolatedConfig(&config);
    config.site_import = 0;
    config.write_bytecode = 0;
    config.parse_argv = 0;
    config.module_search_paths_set = 1;
    config.use_hash_seed = 1;
    config.hash_seed = 0;
    status = PyConfig_SetString(&config, &config.program_name, L"jot-python");
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    status = PyConfig_SetBytesString(&config, &config.run_command, submission);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    status = Py_InitializeFromConfig(&config);
    PyConfig_Clear(&config);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    return Py_RunMain();
}
"""
    )
    output.write_text(program, encoding="utf-8")
    return len(modules)


def uleb(value):
    encoded = bytearray()
    while value >= 128:
        encoded.append((value & 127) | 128)
        value >>= 7
    encoded.append(value)
    return encoded


def add_metadata(output):
    wasm = bytearray(output.read_bytes())
    if not wasm.startswith(b"\0asm\x01\0\0\0"):
        raise ValueError("expected a core Wasm module")
    offset = wasm.find(SOURCE_BEGIN)
    if offset < 0 or wasm.find(SOURCE_BEGIN, offset + 1) != -1:
        raise ValueError("template must contain exactly one source slot")
    tail = offset + SOURCE_CAPACITY
    if wasm[tail:tail + len(SOURCE_END)] != SOURCE_END:
        raise ValueError("source slot was not preserved by the linker")
    metadata = (
        uleb(len(METADATA_NAME))
        + METADATA_NAME
        + struct.pack("<II", offset, SOURCE_CAPACITY)
    )
    wasm += b"\0" + uleb(len(metadata)) + metadata
    output.write_bytes(wasm)
    output.chmod(0o644)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    compiler = parser.add_mutually_exclusive_group(required=True)
    compiler.add_argument("--wasi-sdk", type=Path)
    compiler.add_argument("--cc", type=Path)
    parser.add_argument("--sysroot", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.cc and not args.sysroot:
        parser.error("--cc requires --sysroot")
    source, build, output = (
        value.resolve()
        for value in (args.source_dir, args.build_dir, args.output)
    )
    cc = args.wasi_sdk.resolve() / "bin/clang" if args.wasi_sdk else args.cc.resolve()
    sysroot_flags = [f"--sysroot={args.sysroot.resolve()}"] if args.sysroot else []
    version_header = (source / "Include/patchlevel.h").read_text()
    source_version = re.search(r'#define PY_VERSION\s+"([^"]+)"', version_header)
    if source_version is None or source_version[1] != sys.version.split()[0]:
        raise ValueError("native Python and WASI CPython source versions must match")
    if sys.version_info[:2] != (3, 14):
        raise ValueError("this template build targets CPython 3.14")

    output.parent.mkdir(parents=True, exist_ok=True)
    c_main = output.with_suffix(".c")
    count = generate_main(source, c_main)
    subprocess.run(
        [
            str(cc), "--target=wasm32-wasip1", *sysroot_flags, "-O2",
            "-I" + str(source / "Include"), "-I" + str(build), str(c_main),
            str(build / "libpython3.14.a"),
            *map(str, sorted((build / "Modules").rglob("*.a"))),
            "-lm", "-ldl", "-lwasi-emulated-getpid", "-lwasi-emulated-signal",
            "-lwasi-emulated-process-clocks", "-Wl,-z,stack-size=1048576",
            "-Wl,--strip-debug", "-o", str(output),
        ],
        check=True,
    )
    add_metadata(output)
    print(f"{output}: {output.stat().st_size} bytes, {count} frozen modules")


if __name__ == "__main__":
    main()
