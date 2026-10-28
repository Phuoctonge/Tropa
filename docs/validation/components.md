# Tropa baseline component inventory

Date: 2026-10-27. Inventory of source and downloaded build inputs, not a release license audit.

| Component | Actual revision/version | Source and status |
|---|---|---|
| Shell | v2.5.8 / 4337bd85758d54d7b9044b2da3c9ac1176eae1f3 | clash-verge-rev/clash-verge-rev, upstream history imported; package and Cargo manifests declare GPL-3.0-only |
| Stable mihomo | v1.19.33 | MetaCubeX/mihomo official release, downloaded by upstream prebuild; runtime not tested |
| Alpha mihomo | alpha-5b79f3c | MetaCubeX/mihomo Prerelease-Alpha, upstream build input; Tropa's product requirement remains one active core |
| Service binaries | v2.7.6 | clash-verge-rev/clash-verge-service-ipc release archive; downloaded, not installed or executed |
| JS mihomo plugin | 7a03d16f4d39b1bb885b7babe66b9e153e34b615 | Revision resolved by pnpm-lock.yaml; manifest's main branch is not treated as a pin |
| WarpScout/usque | Not selected or downloaded | Previous design research only; version, CLI and dependency licenses still require validation |
| Wintun | Not inventoried separately yet | Do not infer binary provenance or redistribution permission from support for TUN |
| NSIS SimpleSC | 1.30 | Downloaded by upstream prebuild into project-local build AppData; not a Tropa runtime requirement |

## Downloaded binary SHA256

| File | SHA256 |
|---|---|
| verge-mihomo-x86_64-pc-windows-msvc.exe | 0b4c7353057a78de8a09db445351aadec7d72a4f48adc7d99691f3c5ff74d3da |
| verge-mihomo-alpha-x86_64-pc-windows-msvc.exe | 4d8a5227faa44639987b5708f4fc2b88db26dfef4098f9789b8df56abe1dd938 |
| clash-verge-service.exe | 1f6b12b90f3b68da69352939908e9e36c66a4eefef274c25c3b368e7bd4a56c6 |
| clash-verge-service-install.exe | fa3a36b13d75e775162e29d307b94efcba78f814ba766d755699ee5821189b77 |
| clash-verge-service-uninstall.exe | 10866cbb863ce5fbcd0673306736c44603214aab640dd44a602fd9abc8c6b8b9 |

These hashes identify the bytes obtained; they do not by themselves establish authenticity. Sources above were fetched over verified HTTPS. GPL source availability, notices and the full portable runtime/driver dependency set still require a release audit.

## Developer tooling

Rustup bootstrapper: official `https://win.rustup.rs/x86_64`, SHA256 `00f139da614dced87d3c3d4121c43eebdac3c1bf8a5ee86b9d80dfd71c8d70fd`.

Microsoft Build Tools bootstrapper: official `https://aka.ms/vs/17/release/vs_BuildTools.exe`, valid Microsoft Authenticode signature, SHA256 `4aad1082e180f558598c346eb105b465b4908148cb9f7f0511aa1105620e0c30`.
