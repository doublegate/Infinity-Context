<!-- SPDX-FileCopyrightText: 2026 DoubleGate -->
<!-- SPDX-License-Identifier: CC-BY-NC-ND-4.0 -->

# Raw v1.2.0 qualification evidence (release asset)

The large raw logs and mutation inventories behind the v1.2.0 audit and qualification are not
stored in the repository. They are published as one archive attached to the
[v1.2.0 GitHub release](https://github.com/doublegate/Infinity-Context/releases/tag/v1.2.0):
`icx-v1.2.0-raw-evidence.tar.gz`, listed in that release's `SHA256SUMS`. Extracting it at the
repository root restores each file at the path below. The summaries, triage records and every
file that the documentation links or names remain in the repository.

42 files, 42.4 MB:

| Path | Bytes | SHA-256 |
| :--- | ---: | :--- |
| `docs/codex/evidence/validation-fuzz-ahtl_stream.log` | 1,571,106 | `3dbc4af9c18e966e3e21258d6563e659f37d385282ab27defde0ee5dd39554f0` |
| `docs/codex/evidence/validation-fuzz-mcp_jsonrpc.log` | 686,315 | `5fc4d9f37b587ad638a0b8f113ffa57698d31eda9f75436f52e7fd173e3880b3` |
| `docs/codex/evidence/validation-mutation-icx-adapters-inventory.json` | 1,978,549 | `b8856ea94802fbb82a02afd297e590c256550460d833fd8adb340fa3c25573e0` |
| `docs/codex/evidence/validation-mutation-icx-bus-inventory.json` | 541,932 | `a2ba1ca47a0e9ea42b87b42403176e3fd170b6a6c6ccc79092200b1dcbe98218` |
| `docs/codex/evidence/validation-mutation-icx-cli-inventory.json` | 888,210 | `c8d7761418448b1462ae641e1452a09dea4f32012f4a3405242060178ae2c0c0` |
| `docs/codex/evidence/validation-mutation-icx-cli.log` | 147,047 | `e58f144543bbd10976656dd3d0eecb9d798623beda38ae9258bbaca421c83770` |
| `docs/codex/evidence/validation-mutation-icx-codec-inventory.json` | 2,070,507 | `daa8610a8e2bb3cb89e0fbaf0ac9daaf6a3ebb490f1216ea809f6b1940d3a9f6` |
| `docs/codex/evidence/validation-mutation-icx-core-inventory.json` | 1,014,438 | `f5e5a318848377329674c985a6c37e925bacd9e7f274b802607d291c39381f7d` |
| `docs/codex/evidence/validation-mutation-icx-hdc-inventory.json` | 1,527,005 | `5bff15139495d9abfbc42b07467b7a8dd4157677f7a806dc2aae0994044dad50` |
| `docs/codex/evidence/validation-mutation-icx-proxy-inventory.json` | 1,065,885 | `c6fb3f0cbc2353656daaa8ab41e8872da8715a0141cfc2a35e25e4097cb8105c` |
| `docs/codex/evidence/validation-mutation-icx-proxy.log` | 125,993 | `79b03a83c6035752571e2bd178da8cc71f8fa6dbb1e47658621cf509eb7f92ce` |
| `docs/codex/evidence/validation-mutation-icx-store-inventory.json` | 292,451 | `4d01f2f2ed16f1990f194b3903c2fbc2e13dfe0b0863286c9e4d2ce39a64d7c8` |
| `docs/releases/v1.2.0/evidence/fuzz-ahtl_stream.log` | 1,617,910 | `41692b98eed9a67831e946b5db94f2906e33469093c725f87158d82c3f837306` |
| `docs/releases/v1.2.0/evidence/fuzz-hdc_snapshot.log` | 170,135 | `7c9666acb833f48db88f078a08012def6772e0a738602aea381ad14f92505967` |
| `docs/releases/v1.2.0/evidence/fuzz-mcp_jsonrpc.log` | 697,293 | `9509894cb3235cd1a929dbddbfe68fa6a385936456ee6a9604a97cc758be72df` |
| `docs/releases/v1.2.0/evidence/fuzz-redact-attempt6.log` | 655,724 | `540792af5bc2a44fe79a3ef299ab0860d3c531ad933fc2fe189cf4d8e3ecb436` |
| `docs/releases/v1.2.0/evidence/fuzz-redact-final.log` | 177,150 | `50d1a42a3012118263f25e551e3d19997e8b1766bf52ee25fcaccd1815625739` |
| `docs/releases/v1.2.0/evidence/fuzz-redact-qualification.log` | 103,108 | `8c93d40dc3ab7c6fa45faae36db14504eb7328e71380b473ec59cf6f1db282ca` |
| `docs/releases/v1.2.0/evidence/fuzz-redact-retry.log` | 371,903 | `a7ea47cb4eae8db522b489149cc322835c6a1a8ac27f5ad7e9733874cdb90e13` |
| `docs/releases/v1.2.0/evidence/mutation-critical-bus-inventory.json` | 556,840 | `97b27be0c35e187af7ff9b66c9275c1e17fe007537a81502bb8bc78206fd64d8` |
| `docs/releases/v1.2.0/evidence/mutation-critical-codec-inventory.json` | 2,140,318 | `17c3e298aa4c95a144a8c15ce6a21f32ba669d6d8c5759913958798b7437dba9` |
| `docs/releases/v1.2.0/evidence/mutation-critical-core-inventory.json` | 1,067,494 | `c91d1c306a9bc26e69c1cc8a3c143b0c33528ca7a7418779095267f4b02f30b6` |
| `docs/releases/v1.2.0/evidence/mutation-critical-hdc-inventory.json` | 1,664,512 | `f50cd259fe64b94c2265a6a9990cb08b1fae1c55040b587c1a28e08a13af0d68` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r2-codec-inventory.json` | 2,127,904 | `b96629f33fc4cfa194226bb9bc12935107891952a0e1df3332929e754a5f441c` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r2-core-inventory.json` | 1,067,494 | `c91d1c306a9bc26e69c1cc8a3c143b0c33528ca7a7418779095267f4b02f30b6` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r2-hdc-inventory.json` | 1,617,411 | `72a7423dccc0e0820b2f81b1bdb4b4f9a5f779b7954a6328086344431600a0b1` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r3-core-inventory.json` | 1,067,494 | `c91d1c306a9bc26e69c1cc8a3c143b0c33528ca7a7418779095267f4b02f30b6` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r4-core-inventory.json` | 1,067,494 | `c91d1c306a9bc26e69c1cc8a3c143b0c33528ca7a7418779095267f4b02f30b6` |
| `docs/releases/v1.2.0/evidence/mutation-critical-r5-core-inventory.json` | 1,067,494 | `c91d1c306a9bc26e69c1cc8a3c143b0c33528ca7a7418779095267f4b02f30b6` |
| `docs/releases/v1.2.0/evidence/mutation-icx-adapters-inventory.json` | 2,001,633 | `b84d21236864cc9ff06f27ee4ea428bca097e9bbfc7efe905a83e988b846d410` |
| `docs/releases/v1.2.0/evidence/mutation-icx-adapters.log` | 174,701 | `5f489949801f52d6b01144697f5b400b897ea6ed53b25132eeee5680b8c18ff5` |
| `docs/releases/v1.2.0/evidence/mutation-icx-bus-inventory.json` | 556,840 | `97b27be0c35e187af7ff9b66c9275c1e17fe007537a81502bb8bc78206fd64d8` |
| `docs/releases/v1.2.0/evidence/mutation-icx-cli-inventory.json` | 901,431 | `37bb06b945a7365e75b59e2a48cc656eed16b20acbce2afbe38749f85b3309f1` |
| `docs/releases/v1.2.0/evidence/mutation-icx-codec-inventory.json` | 2,129,931 | `c45d2d3af920b5691e35f443220d7b7b280efe727cd181ed6fedc5d71af25293` |
| `docs/releases/v1.2.0/evidence/mutation-icx-core-inventory.json` | 1,064,005 | `4ef558506e65a9980334543daa45593939f8ea4df2c822c69bb83a3574793b1c` |
| `docs/releases/v1.2.0/evidence/mutation-icx-hdc-inventory.json` | 1,664,512 | `f50cd259fe64b94c2265a6a9990cb08b1fae1c55040b587c1a28e08a13af0d68` |
| `docs/releases/v1.2.0/evidence/mutation-icx-proxy-inventory.json` | 1,065,885 | `c6fb3f0cbc2353656daaa8ab41e8872da8715a0141cfc2a35e25e4097cb8105c` |
| `docs/releases/v1.2.0/evidence/mutation-icx-proxy.log` | 126,241 | `87911c0eee72d35dce3666159dc03f431dea173e2d95ef48bc0ff0d0d4245f76` |
| `docs/releases/v1.2.0/evidence/mutation-icx-store-inventory.json` | 292,451 | `4d01f2f2ed16f1990f194b3903c2fbc2e13dfe0b0863286c9e4d2ce39a64d7c8` |
| `docs/releases/v1.2.0/evidence/mutation-owned-targeted.log` | 181,251 | `8b6bc14a960bd75b9941d0f8a94e88a9f9213345844b5d2ed033a5928494d5b7` |
| `docs/releases/v1.2.0/evidence/mutation-retry-icx-adapters-inventory.json` | 2,033,244 | `c0d86091eb2e356edf722160a0b2bcc5d884056ab6097630e7b17a055e3d141b` |
| `docs/releases/v1.2.0/evidence/mutation-retry-icx-proxy-inventory.json` | 1,065,885 | `c6fb3f0cbc2353656daaa8ab41e8872da8715a0141cfc2a35e25e4097cb8105c` |
