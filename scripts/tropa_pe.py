"""Read PE headers/resources as bytes; never load or execute the image."""

import hashlib
import struct
import xml.etree.ElementTree as ET


def inspect_pe(data):
    def block(offset, size):
        if offset < 0 or size < 0 or offset + size > len(data):
            raise ValueError("Truncated PE data")
        return data[offset:offset + size]

    def u16(offset):
        return struct.unpack("<H", block(offset, 2))[0]

    def u32(offset):
        return struct.unpack("<I", block(offset, 4))[0]

    if block(0, 2) != b"MZ":
        raise ValueError("Not a Windows PE executable")
    pe = u32(0x3C)
    if block(pe, 4) != b"PE\0\0":
        raise ValueError("Invalid PE signature")
    if u16(pe + 4) != 0x8664:
        raise ValueError("Expected Windows x64 executable")
    characteristics = u16(pe + 22)
    if not characteristics & 2 or characteristics & 0x2000:
        raise ValueError("Expected executable image, not DLL")
    count = u16(pe + 6)
    optional_size = u16(pe + 20)
    optional = pe + 24
    block(optional, optional_size)
    if optional_size < 136 or u16(optional) != 0x20B:
        raise ValueError("Expected PE32+ optional header")
    if u16(optional + 68) != 2:
        raise ValueError("Expected Windows GUI subsystem")
    if u32(optional + 108) < 3:
        raise ValueError("Missing PE resource directory / manifest")
    resource_rva, resource_size = u32(optional + 128), u32(optional + 132)
    if resource_size < 16 or resource_size > 16 * 1024 * 1024:
        raise ValueError("Invalid PE resource directory size")
    if not 1 <= count <= 96:
        raise ValueError("Invalid PE section count")
    sections = []
    for index in range(count):
        section = optional + optional_size + index * 40
        block(section, 40)
        sections.append((u32(section + 12), u32(section + 20), u32(section + 16)))

    def rva_to_offset(rva, size):
        matches = [raw + rva - start for start, raw, length in sections
                   if start <= rva and rva + size <= start + length]
        if len(matches) != 1:
            raise ValueError("Unmapped or ambiguous PE resource address")
        block(matches[0], size)
        return matches[0]

    base = rva_to_offset(resource_rva, resource_size)

    def resource_block(relative, size):
        if relative < 0 or relative + size > resource_size:
            raise ValueError("PE resource outside resource directory")
        block(base + relative, size)
        return base + relative

    def entries(relative):
        header = resource_block(relative, 16)
        count = u16(header + 12) + u16(header + 14)
        if count > 4096:
            raise ValueError("Oversized PE resource table")
        start = resource_block(relative + 16, count * 8)
        return [(u32(start + i * 8), u32(start + i * 8 + 4)) for i in range(count)]

    def directory_child(relative, identifier):
        matches = [target for name, target in entries(relative) if name == identifier]
        if len(matches) != 1 or not matches[0] & 0x80000000:
            raise ValueError("Missing or ambiguous embedded application manifest")
        child = matches[0] & 0x7FFFFFFF
        if child == relative:
            raise ValueError("Loop in PE manifest directory")
        return child

    manifest_dir = directory_child(directory_child(0, 24), 1)
    languages = entries(manifest_dir)
    if not languages or len(languages) > 64 or len({name for name, _ in languages}) != len(languages):
        raise ValueError("Invalid PE manifest language table")
    for name, relative in languages:
        if name & 0x80000000 or relative & 0x80000000:
            raise ValueError("Invalid PE manifest resource leaf")
        leaf = resource_block(relative, 16)
        rva, size = u32(leaf), u32(leaf + 4)
        if not 0 < size <= 1024 * 1024:
            raise ValueError("Invalid embedded manifest size")
        offset = rva_to_offset(rva, size)
        try:
            root = ET.fromstring(block(offset, size))
        except ET.ParseError as error:
            raise ValueError("Invalid embedded XML manifest") from error
        namespace = "{urn:schemas-microsoft-com:asm.v3}"
        levels = root.findall(f"./{namespace}trustInfo/{namespace}security/"
                              f"{namespace}requestedPrivileges/{namespace}requestedExecutionLevel")
        if (len(levels) != 1 or levels[0].get("level") != "requireAdministrator"
                or levels[0].get("uiAccess", "false") != "false"):
            raise ValueError("Embedded manifest must requireAdministrator with uiAccess=false")
    return {
        "architecture": "x64", "subsystem": "windowsGui",
        "executionLevel": "requireAdministrator", "manifestLanguages": len(languages),
        "sha256": hashlib.sha256(data).hexdigest(),
        "executed": False,
    }
