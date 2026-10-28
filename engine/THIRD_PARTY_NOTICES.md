# Search engine source references

The framing and wire implementation is newly written Tropa code. WG/AWG UAPI configuration, single-packet UDP bind, MASQUE enrolled-key TLS/dial-lifetime approach and userspace-device lifecycle adapt the pinned WarpScout sources recorded in source-origins.json. Sessions own their identity, socket and device; no WarpScout CLI/TUI code is imported. The original MIT notice is retained below.

The finite tuning catalog adapts DNS/SIP/STUN CPS packet construction, the default iCloud I1 bytes and SNI candidates from the same pinned WarpScout sources. Tropa uses bounded concrete variants rather than upstream's global mutable and unbounded random tuning loops. The QUIC Initial generator is not incorporated; its profile is explicitly unavailable because provenance/license review remains unresolved. This absence is an open full-parity release gate.

The pinned amneziawg-go, usque/api, connect-ip-go and quic-go libraries are imported directly. The H2 connector adapts the direct-endpoint branch of usque/api/masque.go with explicit TCP cleanup and no environment proxy fallback. Its MIT notice is preserved in licenses/github.com__Diniboy1123__usque/LICENSE.md. Endpoint public-key pinning remains mandatory for H2 and H3. Exact imported module versions and checksums are in dependencies.json, with root license/notice texts under licenses/. This is an implementation-stage inventory, not public-release clearance: the final worker dependency graph, applicable additional source notices and existing Wintun distribution obligations must be checked before packaging.

## WarpScout v0.16.1

MIT License

Copyright (c) 2026 Nikita S.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## warpscout-android

MIT License

Copyright (c) 2026 Nikita S.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
