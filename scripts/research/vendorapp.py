"""Looks inside a controller maker's PC app (.NET) for what its controllers send and how the app
talks to them. See README.md in this folder for the steps that worked on Flydigi's app.

    python -I vendorapp.py unpack SETUP.exe OUT        Inno Setup installer: data stream, programs, symbols
    python -I vendorapp.py resources ASSEMBLY OUT      embedded resources (Costura, .resources, WPF)
    python -I vendorapp.py baml FILE.baml              a WPF string dictionary (translations) as key = text
    python -I vendorapp.py strings ASSEMBLY            every string literal in the code, with its offset
    python -I vendorapp.py methods ASSEMBLY            every type's methods and fields, with tokens
    python -I vendorapp.py pinvoke ASSEMBLY            native functions it calls (hid.dll, setupapi...)
    python -I vendorapp.py dis ASSEMBLY NAME|0x06...   the IL of methods, names resolved
    python -I vendorapp.py callers ASSEMBLY NAME...    methods that call these methods
    python -I vendorapp.py fields ASSEMBLY NAME...     methods that read or write these fields
    python -I vendorapp.py uses ASSEMBLY OFFSET...     methods that load these strings (offsets from `strings`)

Needs dnfile and dncil (requirements.txt). Run it with -I and keep downloads in their own folder:
it only reads them, but an interpreter started inside a downloaded folder could load code from it.
"""

import logging
import lzma
import os
import re
import struct
import sys

logging.disable(logging.CRITICAL)
sys.stdout.reconfigure(encoding="utf-8")


# ---------------------------------------------------------------- installers


def unpack(setup, out):
    """Inno Setup keeps every file in one compressed stream after a `zlb\\x1a` mark. The biggest
    stream is the data: written whole, then every program (PE) and symbol file (PDB) in it is cut
    out, named by its version info."""
    d = open(setup, "rb").read()
    os.makedirs(out, exist_ok=True)
    best = b""
    for m in re.finditer(rb"zlb\x1a", d):
        data = _lzma(d, m.start() + 4)
        print(f"stream at {m.start()}: {len(data)} bytes")
        if len(data) > len(best):
            best = data
    if not best:
        sys.exit("no compressed stream found: not an Inno Setup installer, or another compression")
    open(os.path.join(out, "stream.bin"), "wb").write(best)
    carve(best, os.path.join(out, "pe"), os.path.join(out, "pdb"))


def _lzma(d, at):
    props = d[at : at + 5]
    tries = []
    b = props[0]
    tries.append(([{"id": lzma.FILTER_LZMA2, "dict_size": (2 | (b & 1)) << (b // 2 + 11)}], at + 1))
    pb, b = divmod(b, 45)
    lp, lc = divmod(b, 9)
    tries.append(([{"id": lzma.FILTER_LZMA1, "lc": lc, "lp": lp, "pb": pb, "dict_size": int.from_bytes(props[1:5], "little")}], at + 5))
    best = b""
    for filters, start in tries:
        try:
            dec = lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=filters)
        except (lzma.LZMAError, ValueError):
            continue
        got = []
        pos = start
        while pos < len(d) and not dec.eof:
            try:
                got.append(dec.decompress(d[pos : pos + (1 << 20)]))
            except lzma.LZMAError:
                break
            pos += 1 << 20
        data = b"".join(got)
        if len(data) > len(best):
            best = data
    return best


def carve(d, pe_out, pdb_out):
    os.makedirs(pe_out, exist_ok=True)
    os.makedirs(pdb_out, exist_ok=True)
    for m in re.finditer(rb"MZ", d):
        o = m.start()
        try:
            pe = struct.unpack_from("<I", d, o + 0x3C)[0]
            if pe > 0x1000 or d[o + pe : o + pe + 4] != b"PE\0\0":
                continue
            sections = struct.unpack_from("<H", d, o + pe + 6)[0]
            opt = struct.unpack_from("<H", d, o + pe + 20)[0]
            table = o + pe + 24 + opt
            end = max(sum(struct.unpack_from("<II", d, table + i * 40 + 16)) for i in range(sections))
            magic = struct.unpack_from("<H", d, o + pe + 24)[0]
            dirs = o + pe + 24 + (96 if magic == 0x10B else 112)
            cert, size = struct.unpack_from("<II", d, dirs + 4 * 8)
            if cert and end < cert + size < end + 1_000_000:
                end = cert + size
        except struct.error:
            continue
        blob = d[o : o + end]
        name = _version_string(blob, "OriginalFilename") or "noname"
        net = "_NET" if b"BSJB" in blob else ""
        path = os.path.join(pe_out, f"{o:09d}_{name}{net}.bin".replace("/", "_"))
        open(path, "wb").write(blob)
        print(path, len(blob))
    # Symbol files name every local variable of every method: shipped by mistake, they make the
    # code far easier to read.
    sig = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0"
    for m in re.finditer(re.escape(sig), d):
        block, _, blocks = struct.unpack_from("<III", d, m.start() + 32)
        blob = d[m.start() : m.start() + block * blocks]
        path = os.path.join(pdb_out, f"{m.start():09d}.pdb")
        open(path, "wb").write(blob)
        print(path, len(blob))


def _version_string(blob, key):
    # The program's own version info comes last: embedded libraries before it carry theirs.
    k = blob.rfind(key.encode("utf-16le"))
    if k < 0:
        return ""
    s = blob[k + 2 * len(key) + 2 : k + 2 * len(key) + 202]
    s = s[: s.find(b"\0\0\0") + 1]
    return s.decode("utf-16le", "ignore").strip("\0").strip()


# ---------------------------------------------------------------- resources


def _r7(d, p):
    v = s = 0
    while True:
        b = d[p]
        p += 1
        v |= (b & 0x7F) << s
        s += 7
        if b < 0x80:
            return v, p


def resources(assembly, out):
    """The assembly's manifest resources, Costura's embedded libraries unpacked, and each
    `.resources` split into its entries: files for streams, one text file for strings."""
    import zlib

    pe = _open(assembly)
    os.makedirs(out, exist_ok=True)
    base = pe.get_offset_from_rva(pe.net.struct.ResourcesRva)
    data = pe.__data__
    for r in pe.net.mdtables.ManifestResource.rows:
        name = str(r.Name)
        off = base + r.Offset
        n = struct.unpack_from("<I", data, off)[0]
        blob = bytes(data[off + 4 : off + 4 + n])
        if name.endswith(".compressed"):
            for bits in (-15, 15, 31):  # Costura's raw deflate, else zlib or gzip
                try:
                    blob, name = zlib.decompress(blob, bits), name[: -len(".compressed")]
                    break
                except zlib.error as e:
                    error = e
            else:
                print(f"{name}: kept compressed ({error})", file=sys.stderr)
        path = os.path.join(out, name)
        open(path, "wb").write(blob)
        print(path, len(blob))
        if name.endswith(".resources"):
            _split_resources(blob, path + ".d")


def _split_resources(d, out):
    if struct.unpack_from("<I", d, 0)[0] != 0xBEEFCACE:
        return
    os.makedirs(out, exist_ok=True)
    _, skip = struct.unpack_from("<II", d, 4)
    p = 12 + skip
    _, n, ntypes = struct.unpack_from("<III", d, p)
    p += 12
    for _ in range(ntypes):
        length, p = _r7(d, p)
        p += length
    p = (p + 7) & ~7
    p += 4 * n
    positions = struct.unpack_from(f"<{n}I", d, p)
    p += 4 * n
    data_start = struct.unpack_from("<I", d, p)[0]
    names = p + 4
    strings = []
    for pos in positions:
        q = names + pos
        length, q = _r7(d, q)
        name = d[q : q + length].decode("utf-16le")
        at = data_start + struct.unpack_from("<I", d, q + length)[0]
        kind, q = _r7(d, at)
        if kind == 1:  # String
            length, q = _r7(d, q)
            strings.append(f"{name} = {d[q : q + length].decode('utf-8', 'replace')!r}")
        elif kind in (0x20, 0x21):  # byte array, stream
            length = struct.unpack_from("<I", d, q)[0]
            open(os.path.join(out, name.replace("/", "__")), "wb").write(d[q + 4 : q + 4 + length])
    if strings:
        open(os.path.join(out, "_strings.txt"), "w", encoding="utf-8").write("\n".join(sorted(strings)))
    print(f"{out}: {n} entries")


# Fixed sizes of the BAML records a string dictionary uses; every other record starts with its size.
_BAML_FIXED = {1: 6, 2: 0, 3: 3, 4: 0, 7: 2, 8: 0, 9: 2, 10: 0, 11: 2, 12: 0, 13: 2, 14: 0, 26: 0, 37: 4, 39: 9, 40: 9, 41: 0, 42: 0, 43: 0, 44: 2, 45: 4, 47: 3, 48: 3, 49: 0, 50: 2, 53: 8, 54: 4, 55: 3}


def baml(path):
    """A compiled `<ResourceDictionary>` of `<sys:String x:Key="...">`: the app's text in one
    language. The keys say what each text is for (connect_switch_mode_apex2...)."""
    d = open(path, "rb").read()
    p = 4 + struct.unpack_from("<I", d, 0)[0] + 12
    strings, keys, texts = {}, [], []
    while p < len(d):
        t = d[p]
        p += 1
        if t in _BAML_FIXED:
            p += _BAML_FIXED[t]
            continue
        size, q = _r7(d, p)
        end = p + size
        if t == 32:  # StringInfo
            sid = struct.unpack_from("<H", d, q)[0]
            length, s = _r7(d, q + 2)
            strings[sid] = d[s : s + length].decode("utf-8", "replace")
        elif t == 38:  # DefAttributeKeyString
            keys.append(struct.unpack_from("<H", d, q)[0])
        elif t in (16, 17, 51):  # Text
            length, s = _r7(d, q)
            texts.append(d[s : s + length].decode("utf-8", "replace"))
        p = end
    if len(keys) != len(texts):
        print(f"warning: {len(keys)} keys for {len(texts)} texts, pairs may be off", file=sys.stderr)
    for sid, text in zip(keys, texts):
        print(f"{strings.get(sid, f'#{sid}')} = {text!r}")


# ---------------------------------------------------------------- code


def _open(assembly):
    import dnfile

    return dnfile.dnPE(assembly)


def strings(assembly):
    pe = _open(assembly)
    us = pe.net.user_strings
    data = us.get_data_bytes() if hasattr(us, "get_data_bytes") else us.__data__
    i = 1
    while i < len(data):
        b = data[i]
        if b & 0x80 == 0:
            n, h = b, 1
        elif b & 0xC0 == 0x80:
            n, h = ((b & 0x3F) << 8) | data[i + 1], 2
        else:
            n, h = ((b & 0x1F) << 24) | (data[i + 1] << 16) | (data[i + 2] << 8) | data[i + 3], 4
        s = data[i + h : i + h + n - 1] if n else b""
        print(f"{i:06x}\t{s.decode('utf-16le', 'replace')!r}")
        i += h + n


def methods(assembly):
    pe = _open(assembly)
    for t in pe.net.mdtables.TypeDef.rows:
        for m in t.MethodList or []:
            print(f"{t.TypeNamespace}.{t.TypeName}::{m.row.Name}\trva={m.row.Rva:#x}\ttok=0x06{m.row_index:06x}")
        for f in t.FieldList or []:
            print(f"FIELD {t.TypeNamespace}.{t.TypeName}::{f.row.Name}\ttok=0x04{f.row_index:06x}")


def pinvoke(assembly):
    md = _open(assembly).net.mdtables
    for r in md.ImplMap.rows if md.ImplMap else []:
        try:
            print(f"{r.ImportScope.row.Name}!{r.ImportName}\tas {r.MemberForwarded.row.Name}")
        except AttributeError:
            pass


def _instructions(pe, rva):
    from dncil.cil.body import CilMethodBody
    from dncil.cil.body.reader import CilMethodBodyReaderBase

    class Reader(CilMethodBodyReaderBase):
        def __init__(self):
            self.off = pe.get_offset_from_rva(rva)

        def read(self, n):
            d = pe.get_data(pe.get_rva_from_offset(self.off), n)
            self.off += n
            return d

        def tell(self):
            return self.off

        def seek(self, o):
            self.off = o
            return o

    return CilMethodBody(Reader()).instructions


def dis(assembly, wanted):
    from dncil.clr.token import StringToken, Token

    pe = _open(assembly)
    md = pe.net.mdtables

    def name(t):
        table, i = t >> 24, t & 0xFFFFFF
        try:
            if table == 0x70:
                return repr(pe.net.user_strings.get(i).value)
            if table == 0x06:
                return "M:" + str(md.MethodDef.rows[i - 1].Name)
            if table == 0x04:
                return "F:" + str(md.Field.rows[i - 1].Name)
            if table == 0x0A:
                r = md.MemberRef.rows[i - 1]
                cls = r.Class.row
                return f"MR:{getattr(cls, 'TypeName', None) or getattr(cls, 'Name', '?')}::{r.Name}"
            if table == 0x01:
                return "T:" + str(md.TypeRef.rows[i - 1].TypeName)
            if table == 0x02:
                return "TD:" + str(md.TypeDef.rows[i - 1].TypeName)
            if table == 0x2B:
                return "MS:" + str(md.MethodSpec.rows[i - 1].Method.row.Name)
        except (IndexError, AttributeError):
            pass
        return f"{t:#x}"

    for i, r in enumerate(md.MethodDef.rows, 1):
        if not r.Rva or not (str(r.Name) in wanted or f"0x06{i:06x}" in wanted):
            continue
        print(f"===== {r.Name} tok=0x06{i:06x} rva={r.Rva:#x}")
        for ins in _instructions(pe, r.Rva):
            op = ins.operand
            text = name(op.value) if isinstance(op, (Token, StringToken)) else "" if op is None else str(op)
            print(f"  {ins.offset:04x} {ins.opcode.name} {text}")


def _bodies(pe):
    """Every method's IL bytes, with the type that has it."""
    md = pe.net.mdtables
    image = pe.get_memory_mapped_image()
    owner = {}
    for t in md.TypeDef.rows:
        for m in t.MethodList or []:
            owner[m.row_index] = str(t.TypeName)
    for i, r in enumerate(md.MethodDef.rows, 1):
        if not r.Rva:
            continue
        try:
            b = image[r.Rva]
            if b & 3 == 2:
                code = image[r.Rva + 1 : r.Rva + 1 + (b >> 2)]
            else:
                size = struct.unpack_from("<I", image, r.Rva + 4)[0]
                header = (struct.unpack_from("<H", image, r.Rva)[0] >> 12) * 4
                code = image[r.Rva + header : r.Rva + header + size]
        except (IndexError, struct.error):
            continue
        yield f"{owner.get(i, '?')}::{r.Name} (0x06{i:06x})", r.Rva, code


# Operand sizes of IL instructions (ECMA-335), enough to find where each one starts.
_OPERAND = {
    **dict.fromkeys(range(0x0E, 0x14), 1),  # ldarg.s ... stloc.s
    0x1F: 1, 0x20: 4, 0x21: 8, 0x22: 4, 0x23: 8,
    0x27: 4, 0x28: 4, 0x29: 4,  # jmp, call, calli
    **dict.fromkeys(range(0x2B, 0x38), 1),  # short branches
    **dict.fromkeys(range(0x38, 0x45), 4),  # branches
    0x6F: 4, 0x70: 4, 0x71: 4, 0x72: 4, 0x73: 4, 0x74: 4, 0x75: 4, 0x79: 4,
    **dict.fromkeys(range(0x7B, 0x82), 4),  # field access, stobj
    0x8C: 4, 0x8D: 4, 0x8F: 4, 0xA3: 4, 0xA4: 4, 0xA5: 4, 0xC2: 4, 0xC6: 4, 0xD0: 4,
    0xDD: 4, 0xDE: 1,  # leave, leave.s
}
_OPERAND_FE = {0x06: 4, 0x07: 4, 0x09: 2, 0x0A: 2, 0x0B: 2, 0x0C: 2, 0x0D: 2, 0x0E: 2, 0x12: 1, 0x15: 4, 0x16: 4, 0x19: 1, 0x1C: 4}


def _starts(code):
    """Where each instruction of a method body starts."""
    starts, p = set(), 0
    while p < len(code):
        starts.add(p)
        op = code[p]
        if op == 0xFE and p + 1 < len(code):
            p += 2 + _OPERAND_FE.get(code[p + 1], 0)
        elif op == 0x45 and p + 5 <= len(code):  # switch: a count, then that many targets
            p += 5 + 4 * struct.unpack_from("<I", code, p + 1)[0]
        else:
            p += 1 + _OPERAND.get(op, 0)
    return starts


def _who_uses(pe, needles):
    """Methods with an instruction that is one of the needles (an opcode and a token). The bytes
    are found by a fast scan, then kept only where an instruction starts: a token's bytes also
    turn up by chance inside other operands."""
    for method, _, code in _bodies(pe):
        found = {n: what for n, what in needles.items() if n in code}
        if not found:
            continue
        starts = _starts(code)
        hits = {what for needle, what in found.items() if any(m.start() in starts for m in re.finditer(re.escape(needle), code))}
        if hits:
            print(method, "->", ", ".join(sorted(hits)))


def callers(assembly, names):
    pe = _open(assembly)
    rows = pe.net.mdtables.MethodDef.rows
    needles = {}
    for i, r in enumerate(rows, 1):
        if str(r.Name) in names:
            token = struct.pack("<I", 0x06000000 | i)
            for op in (b"\x28", b"\x6f", b"\x73", b"\xfe\x06", b"\xfe\x07"):  # call, callvirt, newobj, ldftn, ldvirtftn
                needles[op + token] = str(r.Name)
    _who_uses(pe, needles)


def fields(assembly, names):
    pe = _open(assembly)
    needles = {}
    for i, f in enumerate(pe.net.mdtables.Field.rows, 1):
        if str(f.Name) in names:
            token = struct.pack("<I", 0x04000000 | i)
            for op in range(0x7B, 0x81):  # ldfld, ldflda, stfld, ldsfld, ldsflda, stsfld
                needles[bytes([op]) + token] = str(f.Name)
    _who_uses(pe, needles)


def uses(assembly, offsets):
    pe = _open(assembly)
    _who_uses(pe, {b"\x72" + struct.pack("<I", 0x70000000 | int(o, 16)): o for o in offsets})


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    command, target, rest = argv[1], argv[2], argv[3:]
    commands = {
        "unpack": lambda: unpack(target, rest[0]),
        "resources": lambda: resources(target, rest[0]),
        "baml": lambda: baml(target),
        "strings": lambda: strings(target),
        "methods": lambda: methods(target),
        "pinvoke": lambda: pinvoke(target),
        "dis": lambda: dis(target, set(rest)),
        "callers": lambda: callers(target, set(rest)),
        "fields": lambda: fields(target, set(rest)),
        "uses": lambda: uses(target, rest),
    }
    if command not in commands:
        sys.exit(__doc__)
    commands[command]()


if __name__ == "__main__":
    main(sys.argv)
