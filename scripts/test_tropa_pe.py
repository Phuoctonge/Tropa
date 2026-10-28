import struct
import unittest

from tropa_pe import inspect_pe


def fixture(level="requireAdministrator", *, subsystem=2, machine=0x8664):
    manifest = (
        '<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">'
        '<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>'
        f'<requestedExecutionLevel level="{level}" uiAccess="false"/>'
        '</requestedPrivileges></security></trustInfo></assembly>'
    ).encode()
    data = bytearray(0x800)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, machine, 1)
    struct.pack_into("<HH", data, 0x94, 0xF0, 2)
    optional = 0x98
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<H", data, optional + 68, subsystem)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 128, 0x1000, 0x600)
    section = optional + 0xF0
    data[section:section + 8] = b".rsrc\0\0\0"
    struct.pack_into("<IIII", data, section + 8, 0x600, 0x1000, 0x600, 0x200)
    for directory, identifier, target in [(0, 24, 0x20), (0x20, 1, 0x40), (0x40, 1033, 0x60)]:
        struct.pack_into("<H", data, 0x200 + directory + 14, 1)
        struct.pack_into("<II", data, 0x200 + directory + 16, identifier,
                         target | (0x80000000 if directory != 0x40 else 0))
    struct.pack_into("<II", data, 0x260, 0x1100, len(manifest))
    data[0x300:0x300 + len(manifest)] = manifest
    return bytes(data)


class PortablePeTests(unittest.TestCase):
    def test_accepts_gui_x64_with_embedded_admin_manifest(self):
        report = inspect_pe(fixture())
        self.assertEqual(report["architecture"], "x64")
        self.assertEqual(report["subsystem"], "windowsGui")
        self.assertEqual(report["executionLevel"], "requireAdministrator")

    def test_rejects_console_wrong_architecture_and_unelevated_manifest(self):
        for data in [fixture(subsystem=3), fixture(machine=0x14C), fixture("asInvoker")]:
            with self.subTest(header=data[0x84:0x86]):
                with self.assertRaises(ValueError):
                    inspect_pe(data)

    def test_rejects_missing_manifest_and_unmapped_resource_data(self):
        data = bytearray(fixture())
        struct.pack_into("<I", data, 0x210, 10)
        with self.assertRaisesRegex(ValueError, "manifest"):
            inspect_pe(data)
        data = bytearray(fixture())
        struct.pack_into("<I", data, 0x260, 0x70000000)
        with self.assertRaises(ValueError):
            inspect_pe(data)

    def test_rejects_truncated_headers_and_resource_loops(self):
        with self.assertRaises(ValueError):
            inspect_pe(fixture()[:100])
        data = bytearray(fixture())
        struct.pack_into("<I", data, 0x214, 0x80000000)
        with self.assertRaises(ValueError):
            inspect_pe(data)


if __name__ == "__main__":
    unittest.main()
