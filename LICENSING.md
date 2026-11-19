# Licensing & Intellectual Property Architecture

**Infinity-Context (`icx`)** uses a multi-layered **Strong Copyleft + Non-Commercial Specification + Commercial/Patent Dual-Licensing** framework designed to allow free open-source use by the public while strictly protecting the underlying codebase, specifications, and patentable inventions from unauthorized corporate strip-mining or closed-source cloud monetization.

---

## 1. Component Licensing Matrix

| Asset / Directory | License | SPDX Identifier | File Reference |
| :--- | :--- | :--- | :--- |
| **Rust Workspace & CLI (`crates/icx-{core,hdc,codec,store,bus,cli}`, `scripts/`)** | GNU General Public License v3.0 or later | `GPL-3.0-or-later` | [`LICENSE`](LICENSE), [`LICENSES/GPL-3.0-or-later.txt`](LICENSES/GPL-3.0-or-later.txt) |
| **Network-facing crates (`crates/icx-proxy`, `crates/icx-adapters`, which contains `McpPagerServer`)** | GNU General Public License v3.0 or later AND GNU Affero General Public License v3.0 or later | `GPL-3.0-or-later AND AGPL-3.0-or-later` | [`LICENSES/AGPL-3.0-or-later.txt`](LICENSES/AGPL-3.0-or-later.txt) |
| **Architectural Specifications, Novel Mathematical Derivations & Diagrams (`docs/`, `ref-docs/`, `images/`, `assets/`)** | Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International + Patent Reservation | `CC-BY-NC-ND-4.0` | [`LICENSES/CC-BY-NC-ND-4.0.txt`](LICENSES/CC-BY-NC-ND-4.0.txt), [`PATENTS.md`](PATENTS.md) |
| **Proprietary Corporate Products, Closed-Source SaaS/Cloud Services & Clean-Room Commercial Implementations** | Infinity-Context Commercial & Patent License | `LicenseRef-InfinityContext-Commercial` | [`COMMERCIAL.md`](COMMERCIAL.md), [`PATENTS.md`](PATENTS.md) |

---

## 2. How the Licensing Tiers Work

### A. Public Open-Source Use (`GPL-3.0-or-later` & `AGPL-3.0-or-later`)

- **Individual Developers, Researchers & Open-Source Projects:** You are free to build, run, study, modify, and redistribute **Infinity-Context (`icx`)** under the terms of the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)** ([`LICENSE`](LICENSE)).
- **Network & Cloud SaaS Protection (`AGPL-3.0-or-later`):** `crates/icx-proxy` and `crates/icx-adapters` (which contains `McpPagerServer`) are additionally licensed under the **GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`)** ([`LICENSES/AGPL-3.0-or-later.txt`](LICENSES/AGPL-3.0-or-later.txt)). Section 13 of the AGPL requires that, if you **modify** that code and let users interact with it over a computer network, you offer those users the Corresponding Source of your modified version. Section 13 of the GPL-3.0 does not impose a network obligation of its own; it only permits combining GPL code with AGPL code. The GPL-only crates (`icx-core`, `icx-hdc`, `icx-codec`, `icx-store`, `icx-bus`, `icx-cli`) therefore carry no network-use obligation unless they are combined with the AGPL crates, as the `icx` binary is. A [Commercial License](COMMERCIAL.md) is available for uses these terms do not permit.
- **User Repository Boundary:** Running the unmodified `icx` binary locally on your workstation to compress and index context while working on your own projects does **not** place your unrelated project source code under the GPL/AGPL; your own project code retains its own license.

### B. Specification, Patent-Pending & Anti-Clean-Room Protection (`CC-BY-NC-ND-4.0` + `PATENTS.md`)

- The technical specifications in [`docs/`](docs/), research syntheses and mathematical derivations in [`ref-docs/`](ref-docs/), and visual diagrams in [`images/`](images/) and [`assets/`](assets/) are licensed under **`CC-BY-NC-ND-4.0`** ([`LICENSES/CC-BY-NC-ND-4.0.txt`](LICENSES/CC-BY-NC-ND-4.0.txt)) and subject to [`PATENTS.md`](PATENTS.md).
- They may be read and shared for non-commercial open-source and academic purposes with attribution, but **may not** be used commercially or adapted into derivative commercial specifications or proprietary "clean-room" re-implementations without a [Commercial & Patent License](COMMERCIAL.md).

### C. Corporate & Enterprise Commercial Licensing (`COMMERCIAL.md` & `PATENTS.md`)

- Because the Project Owner holds the copyright and patent-pending intellectual property rights (and all external contributions are governed by the [Contributor License Agreement (`CLA.md`)](CLA.md)), the Project Owner offers paid **Commercial & Patent Licenses** to corporations, AI labs, and enterprise organizations that wish to use, embed, host, or commercialize **Infinity-Context (`icx`)** without `GPL-3.0` / `AGPL-3.0` / `CC-BY-NC-ND-4.0` restrictions.
- See [`COMMERCIAL.md`](COMMERCIAL.md) and [`PATENTS.md`](PATENTS.md) for full details.
