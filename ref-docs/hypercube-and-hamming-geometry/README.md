<!--
SPDX-FileCopyrightText: 2026 DoubleGate
SPDX-License-Identifier: CC-BY-NC-ND-4.0 AND LicenseRef-InfinityContext-Commercial
-->

# Hypercube Geometry ($\gamma_n$ / $Q_n$), Hamming Spaces, and Contractive Sub-Hypercube Indexing

**Primary Archival Sources:**
- Wikipedia — *Hypercube*: <https://en.wikipedia.org/wiki/Hypercube> ([local HTML snapshot](wikipedia_hypercube.html), [REST v1 HTML](wikipedia_hypercube_rest_v1.html), [MediaWiki parse JSON](wikipedia_hypercube_metadata.json))
- Wikipedia — *Hypercube graph*: <https://en.wikipedia.org/wiki/Hypercube_graph>
- Wikimedia Commons — *Category:Hypercubes*: [local image metadata JSON](images/retrieved/wikimedia_image_metadata.json)

**Infinity-Context (`icx`) Target Specifications & Crates:**
- [`docs/hyperdimensional-and-fractal-index.md`](../../docs/hyperdimensional-and-fractal-index.md) (§§2–3)
- [`ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md`](../novel-methods/non-human-neural-codecs-and-holographic-context.md) (§§2–3)
- [`crates/icx-hdc/src/hv.rs`](../../crates/icx-hdc/src/hv.rs) (`Hypervector`, `HdcBundler`)
- [`crates/icx-hdc/src/fractal.rs`](../../crates/icx-hdc/src/fractal.rs) (`FractalCoord`)
- [`crates/icx-hdc/src/lib.rs`](../../crates/icx-hdc/src/lib.rs) (`HdcAssociativeIndex`)

---

## 1. Executive Summary & Architectural Role in Infinity-Context

In geometry and combinatorics, an **$n$-dimensional hypercube** (also termed an **$n$-cube**, **measure polytope**, or Coxeter $\gamma_n$ polytope) is the closed, compact, convex regular polytope generalizing the $0$-dimensional point ($0$-cube), $1$-dimensional line segment ($1$-cube), $2$-dimensional square ($2$-cube), $3$-dimensional cube ($3$-cube), and $4$-dimensional tesseract ($4$-cube) to arbitrary dimension $n \ge 0$. Its $1$-skeleton is the **hypercube graph $Q_n$**, whose $2^n$ vertices are labeled by $n$-bit binary strings $\{0, 1\}^n$ and whose $n 2^{n-1}$ edges connect pairs of vertices that differ in exactly one coordinate (Hamming distance $d_H = 1$).

Within **Infinity-Context (`icx`) Plane 3 (`crates/icx-hdc`)**, the geometry of the $n$-dimensional hypercube is not merely an abstract analogy—it is the exact mathematical substrate of two coupled sub-microsecond indexing engines:

1. **The Discrete 10,240-Bit Boolean Hypercube ($Q_{10240} = \{0, 1\}^{10240}$, `Hypervector([u64; 160])`):**
   Every symbol, AST path, and composite event in `icx-hdc` is represented as a vertex of the $10,240$-dimensional Boolean hypercube $Q_{10240}$. The graph geodesic distance between two vertices in $Q_{10240}$ is their exact Hamming distance $d_H(A, B) = \sum_{w=0}^{159} \operatorname{popcnt}(A[w] \oplus B[w])$. Bitwise XOR binding ($A \oplus B$) and cyclic coordinate permutation ($\Pi^k$) are isometric automorphisms belonging to the hyperoctahedral symmetry group $B_{10240} = (\mathbb{Z}_2)^{10240} \rtimes S_{10240}$, and concentration of measure around the equator ($\mu = 5,120$, $\sigma = \sqrt{2,560} \approx 50.596\text{ bits}$) guarantees quasi-orthogonality of independent seeds.
2. **The Continuous 16-Dimensional Centered Hypercube ($[-1, +1]^{16}$, `FractalCoord`):**
   The Fractal Chaos-Game Trajectory Archive (`FCGA`) projects each discrete state `Hypervector` onto one of the $2^{16} = 65,536$ vertices $\{-1, +1\}^{16}$ of the origin-centered $16$-cube $[-1, +1]^{16}$ and evolves a contractive Iterated Function System (IFS) trajectory $z_t = \frac{1}{2} z_{t-1} + \frac{1}{2} v(s_t)$ inside $[-1, +1]^{16}$. Because the closed unit ball of the Chebyshev ($L_\infty$) metric *is* the unit hypercube, a shared $k$-step suffix bounds the $L_\infty$ distance $\|z_a - z_b\|_\infty \le 2^{1-k}$, providing an $O(1)$ ($< 10\text{ ns}$) upper-bound suffix-depth proximity heuristic $L_{\text{shared}} = \lfloor -\log_2(\frac{1}{2}\|z_a - z_b\|_\infty) \rfloor$.

![Infinity-Context Dual Hypercube Geometries](images/generated/icx_hdc_10240_hypercube_and_fcga.png)

---

## 2. Mathematical Synthesis of the $n$-Dimensional Hypercube ($\gamma_n$ / $Q_n$)

### 2.1 Minkowski Sum & Dimensional Sweeping Construction

An $n$-dimensional hypercube of side length $s > 0$ is constructed inductively by sweeping an $(n-1)$-dimensional hypercube a distance $s$ along a new orthogonal basis vector $e_n \perp \operatorname{span}(e_1, \dots, e_{n-1})$. Formally, let $I_i = [0, s e_i]$ denote the line segment of length $s$ along the $i$-th standard basis vector $e_i \in \mathbb{R}^n$. Then the $n$-cube is the **Minkowski sum** of $n$ mutually orthogonal line segments:

$$\gamma_n(s) = I_1 \oplus I_2 \oplus \cdots \oplus I_n = \left\{ \sum_{i=1}^n \lambda_i s e_i \;\middle|\; \lambda_i \in [0, 1] \right\}$$

Because $\gamma_n$ is a Minkowski sum of line segments, every hypercube is a **zonotope** and an **$n$-orthotope** (hyperrectangle with equal edge lengths).

![Minkowski Sum Progression from 0-Cube to 4-Cube](images/generated/0d_to_4d_hypercube_progression.png)

| Dimension $n$ | Polytope Name | Greek / Coxeter Name | Geometric Sweeping Operation |
| :---: | :--- | :--- | :--- |
| **$0$** | **$0$-cube** (Point) | Monon ($\gamma_0$) | Single point $\{0\}$ of dimension zero |
| **$1$** | **$1$-cube** (Line segment) | Dion ($\gamma_1$) | Point swept 1 unit along $e_1$ |
| **$2$** | **$2$-cube** (Square) | Tetragon ($\gamma_2$) | Line segment swept 1 unit along $e_2 \perp e_1$ |
| **$3$** | **$3$-cube** (Cube) | Hexahedron ($\gamma_3$) | Square swept 1 unit along $e_3 \perp \operatorname{span}(e_1, e_2)$ |
| **$4$** | **$4$-cube** (Tesseract) | Octachoron / 8-cell ($\gamma_4$) | Cube swept 1 unit into the 4th dimension along $e_4$ |
| **$5$** | **$5$-cube** | Penteract / Deca-5-tope ($\gamma_5$) | Tesseract swept 1 unit along $e_5$ |

![Dimension Levels 0D to 4D](images/retrieved/Dimension_levels.png)

### 2.2 Dual Coordinate Realizations: Binary $[0, 1]^n$ vs. Bipolar $[-1, +1]^n$

Two canonical Cartesian realizations of the $n$-cube are standard in geometry and coding theory:

1. **The Unit Boolean Hypercube ($[0, 1]^n$):**
   The convex hull of the $2^n$ binary points $\{0, 1\}^n \subset \mathbb{R}^n$:
   $$C_{[0,1]}^n = \operatorname{conv}\!\big(\{0, 1\}^n\big) = [0, 1]^n = \underbrace{[0, 1] \times [0, 1] \times \cdots \times [0, 1]}_{n\text{ times}}$$
   - **Edge length:** $s = 1$
   - **Vertices ($0$-faces):** $V = \{0, 1\}^n$, $|V| = 2^n$
   - **Euclidean ($L_2$) main diagonal:** $d_2 = \sqrt{\sum_{i=1}^n 1^2} = \sqrt{n}$
   - **$L_1$ (Manhattan / Hamming) diameter:** $d_1 = n$
   - **$n$-volume (hypervolume):** $\operatorname{Vol}_n([0, 1]^n) = 1^n = 1$
   - **Boundary $(n-1)$-volume:** $\operatorname{Vol}_{n-1}(\partial [0, 1]^n) = 2n$

2. **The Origin-Centered Bipolar Hypercube ($[-1, +1]^n$):**
   The convex hull of the $2^n$ signed bipolar points $\{\pm 1\}^n = \{-1, +1\}^n \subset \mathbb{R}^n$:
   $$C_{[-1,+1]}^n = \operatorname{conv}\!\big(\{-1, +1\}^n\big) = [-1, +1]^n = \left\{ z \in \mathbb{R}^n \;\middle|\; \|z\|_\infty \le 1 \right\}$$
   - **Edge length:** $s = 2$
   - **Vertices ($0$-faces):** $V = \{-1, +1\}^n$, $|V| = 2^n$
   - **Euclidean ($L_2$) main diagonal:** $d_2 = 2\sqrt{n}$
   - **Chebyshev ($L_\infty$) diameter:** $d_\infty = \max_{u, v \in [-1, +1]^n} \|u - v\|_\infty = 2$
   - **$n$-volume (hypervolume):** $\operatorname{Vol}_n([-1, +1]^n) = 2^n$
   - **Boundary $(n-1)$-volume:** $\operatorname{Vol}_{n-1}(\partial [-1, +1]^n) = 2n \cdot 2^{n-1} = n 2^n$

3. **Bijective Affine Isomorphism ($\phi: [0, 1]^n \leftrightarrow [-1, +1]^n$):**
   $$\phi(b)_i = 2 b_i - 1 \in [-1, +1], \qquad \phi^{-1}(z)_i = \frac{z_i + 1}{2} \in [0, 1]$$
   On the vertex set $\{0, 1\}^n \leftrightarrow \{-1, +1\}^n$, this bijection maps bitwise XOR (`^`) in $(\mathbb{Z}_2, \oplus)$ to coordinate-wise multiplication in $(\{+1, -1\}, \times)$ via $(-1)^{a_i \oplus b_i} = (-1)^{a_i} (-1)^{b_i}$, and scales Euclidean squared distance by a factor of $4$:
   $$\|\phi(a) - \phi(b)\|_2^2 = \sum_{i=1}^n (2a_i - 2b_i)^2 = 4 \sum_{i=1}^n |a_i - b_i| = 4 \, d_H(a, b)$$

### 2.3 Face Lattice, $k$-Face Counts $E_{k,n}$, and Generating Polynomial $(2x+1)^n$

Every boundary face of an $n$-cube is itself a lower-dimensional $k$-cube ($0 \le k \le n$). In the binary coordinate representation $\{0, 1\}^n$, a **$k$-dimensional face ($k$-face)** is uniquely specified by choosing $k$ "free" coordinates (which vary over $[0, 1]$) and fixing the remaining $n - k$ coordinates to either $0$ or $1$.

Hence, the number of $k$-dimensional faces $E_{k,n}$ of an $n$-dimensional hypercube ([OEIS A038207](https://oeis.org/A038207)) is:

$$E_{k,n} = \binom{n}{k} 2^{n-k} = \frac{n!}{k!\,(n-k)!} 2^{n-k} \qquad (0 \le k \le n)$$

#### Combinatorial Proof & Linear Recurrence
- **Vertex-Incidence Proof:** Each of the $2^n$ vertices has $n$ incident edges, so there are $\binom{n}{k}$ ways to choose $k$ mutually orthogonal edges incident to a given vertex to span a $k$-face. Since every $k$-cube contains $2^k$ vertices, each $k$-face is counted $2^k$ times across all $2^n$ vertices, yielding $E_{k,n} = \frac{2^n}{2^k} \binom{n}{k} = 2^{n-k} \binom{n}{k}$.
- **Sweeping Linear Recurrence:** When sweeping an $(n-1)$-cube to form an $n$-cube, every existing $k$-face is duplicated in the initial and final copies ($2 E_{k,n-1}$), and every $(k-1)$-face sweeps out a new $k$-face ($E_{k-1,n-1}$):
  $$E_{k,n} = 2 E_{k,n-1} + E_{k-1,n-1}, \qquad E_{0,0} = 1, \quad E_{k,n} = 0 \text{ for } k < 0 \text{ or } k > n$$
- **Extended $f$-Vector Generating Polynomial:** By the binomial theorem, the $f$-vector $(E_{0,n}, E_{1,n}, \dots, E_{n,n})$ is the coefficient sequence of:
  $$P_n(x) = (2 + x)^n = \sum_{k=0}^n \binom{n}{k} 2^{n-k} x^k = \sum_{k=0}^n E_{k,n} x^k$$
  Evaluating at $x = -1$ immediately yields the **Euler characteristic** of the $n$-cube:
  $$\sum_{k=0}^n (-1)^k E_{k,n} = (2 - 1)^n = 1^n = 1 \implies \chi(\partial \gamma_n) = 1 - (-1)^n = \begin{cases} 0 & \text{if } n \text{ is even} \\ 2 & \text{if } n \text{ is odd} \end{cases}$$
  Evaluating at $x = 1$ yields the total number of faces of all dimensions ($0 \le k \le n$), $P_n(1) = 3^n$, because each coordinate in $\{-1, 0, +1\}^n$ independently takes value $0$ ($[0]$, fixed at lower bound), $1$ ($[1]$, fixed at upper bound), or $*$ ($[0, 1]$, free interval).

#### Complete Table of $n$-Cube Elements ($n = 0 \dots 10$, plus `icx` Dimensions $n = 16$ and $n = 10,240$)

| $n$ | $n$-Cube | Polytope / Coxeter Name | Schläfli Symbol | $0$-faces (Vertices $2^n$) | $1$-faces (Edges $n 2^{n-1}$) | $2$-faces (Squares) | $3$-faces (Cubes) | $4$-faces (Tesseracts) | $5$-faces | $6$-faces | $7$-faces | $8$-faces | $9$-faces | $10$-faces |
| :-: | :--- | :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **0** | 0-cube | Point / **Monon** ($\gamma_0$) | $(\,)$ | $1$ | — | — | — | — | — | — | — | — | — | — |
| **1** | 1-cube | Line segment / **Dion** ($\gamma_1$) | $\{\,\}$ | $2$ | $1$ | — | — | — | — | — | — | — | — | — |
| **2** | 2-cube | Square / **Tetragon** ($\gamma_2$) | $\{4\}$ | $4$ | $4$ | $1$ | — | — | — | — | — | — | — | — |
| **3** | 3-cube | Cube / **Hexahedron** ($\gamma_3$) | $\{4,3\}$ | $8$ | $12$ | $6$ | $1$ | — | — | — | — | — | — | — |
| **4** | 4-cube | **Tesseract** / **Octachoron** ($\gamma_4$) | $\{4,3,3\}$ | $16$ | $32$ | $24$ | $8$ | $1$ | — | — | — | — | — | — |
| **5** | 5-cube | **Penteract** / **Deca-5-tope** ($\gamma_5$) | $\{4,3,3,3\}$ | $32$ | $80$ | $80$ | $40$ | $10$ | $1$ | — | — | — | — | — |
| **6** | 6-cube | **Hexeract** / **Dodeca-6-tope** ($\gamma_6$) | $\{4,3,3,3,3\}$ | $64$ | $192$ | $240$ | $160$ | $60$ | $12$ | $1$ | — | — | — | — |
| **7** | 7-cube | **Hepteract** / **Tetradeca-7-tope** ($\gamma_7$) | $\{4,3,3,3,3,3\}$ | $128$ | $448$ | $672$ | $560$ | $280$ | $84$ | $14$ | $1$ | — | — | — |
| **8** | 8-cube | **Octeract** / **Hexadeca-8-tope** ($\gamma_8$) | $\{4,3,3,3,3,3,3\}$ | $256$ | $1,024$ | $1,792$ | $1,792$ | $1,120$ | $448$ | $112$ | $16$ | $1$ | — | — |
| **9** | 9-cube | **Enneract** / **Octadeca-9-tope** ($\gamma_9$) | $\{4,3,3,3,3,3,3,3\}$ | $512$ | $2,304$ | $4,608$ | $5,376$ | $4,032$ | $2,016$ | $672$ | $144$ | $18$ | $1$ | — |
| **10** | 10-cube | **Dekeract** / **Icosa-10-tope** ($\gamma_{10}$) | $\{4,3,\dots,3\}$ | $1,024$ | $5,120$ | $11,520$ | $15,360$ | $13,440$ | $8,064$ | $3,360$ | $960$ | $180$ | $20$ | $1$ |
| **16** | **16-cube (`icx` `FractalCoord`)** | **Hexadekeract** ($\gamma_{16}$) | $\{4,3^{14}\}$ | $65,536$ | $524,288$ | $1,966,080$ | $4,587,520$ | $7,454,720$ | $8,945,664$ | $8,200,192$ | $5,857,280$ | $3,294,720$ | $1,464,320$ | $512,512$ |
| **10,240** | **10,240-cube (`icx` `Hypervector`)** | **$Q_{10240}$ ($\gamma_{10240}$)** | $\{4,3^{10238}\}$ | $2^{10240}$ | $10240 \cdot 2^{10239}$ | $\binom{10240}{2} 2^{10238}$ | $\binom{10240}{3} 2^{10237}$ | $\dots$ | $\dots$ | $\dots$ | $\dots$ | $\dots$ | $\dots$ | $\dots$ |

*(Note: The edge count sequence $E_{1,n} = n 2^{n-1} = 0, 1, 4, 12, 32, 80, 192, 448, 1024, 2304, 5120, \dots$ is cataloged as [OEIS A001787](https://oeis.org/A001787).)*

### 2.4 Coxeter Group $B_n$, Polytope Duals, and $(n-1)$-Simplex Face Lattice Isomorphism

1. **Hyperoctahedral Symmetry Group ($B_n$):**
   The full group of Euclidean isometries preserving the centered $n$-cube $[-1, +1]^n$ (and equivalently the graph automorphism group $\operatorname{Aut}(Q_n)$) is the **hyperoctahedral group** $B_n$ (also denoted $C_n$ or $BC_n$), which is the wreath product $\mathbb{Z}_2 \wr S_n$ or semidirect product:
   $$\operatorname{Aut}(Q_n) \cong B_n \cong (\mathbb{Z}_2)^n \rtimes S_n, \qquad |B_n| = 2^n \cdot n!$$
   - The normal subgroup $(\mathbb{Z}_2)^n$ of order $2^n$ consists of independent coordinate reflections $z_i \mapsto -z_i$ in $[-1, +1]^n$, which corresponds exactly to **bitwise XOR translation** $b \mapsto b \oplus c$ in $\{0, 1\}^n$.
   - The quotient/complement $S_n$ of order $n!$ consists of all **coordinate axis permutations** $(z_1, \dots, z_n) \mapsto (z_{\pi(1)}, \dots, z_{\pi(n)})$.
   - As an abstract Coxeter group with diagram $\bullet\!\frac{4}{\quad}\!\bullet\!\frac{3}{\quad}\!\bullet \cdots \bullet\!\frac{3}{\quad}\!\bullet$ and Schläfli symbol $\{4, 3, \dots, 3\}$ ($n-2$ threes), $B_n$ is generated by $n$ reflections $r_0, r_1, \dots, r_{n-1}$ satisfying $(r_0 r_1)^4 = 1$ and $(r_i r_{i+1})^3 = 1$ for $i \ge 1$.

2. **Related Regular & Semiregular Polytope Families (Coxeter Classification):**
   For $n \ge 5$, there exist exactly three infinite families of regular convex polytopes in $\mathbb{R}^n$:
   - **$\gamma_n$ (Hypercubes / Measure Polytopes):** Schläfli $\{4, 3^{n-2}\}$, $2^n$ vertices, $2n$ facets (each an $(n-1)$-cube), vertex figure is a regular $(n-1)$-simplex.
   - **$\beta_n$ (Cross-Polytopes / Orthoplexes):** The polar dual of $\gamma_n$, Schläfli $\{3^{n-2}, 4\}$, convex hull of $\{\pm e_1, \dots, \pm e_n\}$ (the unit $L_1$ ball $\|z\|_1 \le 1$), having $2n$ vertices and $2^n$ regular simplex facets.
   - **$\alpha_n$ (Regular Simplices):** Schläfli $\{3^{n-1}\}$, $n+1$ vertices and $n+1$ facets.
   - **$\delta_n$ (Hypercubic Honeycombs):** Schläfli $\{4, 3^{n-2}, 4\}$, the regular Euclidean tessellation $\mathbb{Z}^n$ of $\mathbb{R}^n$ by unit $n$-cubes.
   - **$h\gamma_n$ (Demihypercubes / Half-Measure Polytopes):** Constructed by alternating (deleting every other vertex of $\gamma_n$, retaining the $2^{n-1}$ vertices of even Hamming weight) and bounding the gaps with $2^n$ simplex facets alongside $2n$ demihypercube facets.

3. **Isomorphism with the Hasse Diagram of the $(n-1)$-Simplex Face Lattice:**
   Orienting $Q_n$ vertically from the all-zeros vertex $0^n$ (bottom) to the all-ones vertex $1^n$ (top) stratifies the $2^n$ vertices into $n+1$ Hamming-weight levels $W_k = \{x \in \{0, 1\}^n : \operatorname{wt}(x) = k\}$ of size $\binom{n}{k}$. This directed graph is isomorphic to the **Boolean lattice $\mathcal{P}(\{1, \dots, n\})$** and therefore to the **Hasse diagram of the face lattice of the $(n-1)$-simplex** $\alpha_{n-1}$ (including the empty face $\emptyset$ at $0^n$ and the full $(n-1)$-simplex at $1^n$).

4. **Generalized Complex Hypercubes ($\gamma_n^p$ in $\mathbb{C}^n$):**
   In complex Hilbert space $\mathbb{C}^n$, Coxeter (1974) defined regular complex polytopes $\gamma_n^p = {}_p\{4\}_2\{3\}_2\cdots{}_2\{3\}_2$ whose coordinates take values among the $p$-th roots of unity $\{e^{2\pi i k / p} : 0 \le k < p\}^n$. A $p$-generalized $n$-cube has $p^n$ vertices, $pn$ facets, and $p^{n-m}\binom{n}{m}$ $m$-faces (recovering the real hypercube $\gamma_n^2 = \gamma_n$ at $p = 2$).

### 2.5 Projective Geometry: Perspective, Schlegel, Isometric, and Petrie Polygon Projections

Visualizing an $n$-cube ($n \ge 3$) in $\mathbb{R}^2$ or $\mathbb{R}^3$ uses four mathematically distinct projection operators:

1. **Perspective & Schlegel Projections (`Hexahedron.svg`, `Hypercube.svg`, `Schlegel_wireframe_8-cell.png`):**
   Projects $\gamma_n$ from a viewpoint just outside the center of one of its $(n-1)$-cube facets onto the hyperplane of that facet. For the $4$-cube (tesseract), this embeds an inner $3$-cube concentrically inside an outer $3$-cube with $8$ perspective-foreshortened radial edges connecting corresponding vertices, decomposing the tesseract into $1$ outer cube + $1$ inner cube + $6$ frustum cells = $8$ cubic cells.

   | 3-Cube Perspective (`Hexahedron.svg`) | 4-Cube Perspective (`Hypercube.svg`) | 4-Cube Schlegel Wireframe (`Schlegel_wireframe_8-cell.png`) |
   | :---: | :---: | :---: |
   | ![Hexahedron](images/retrieved/Hexahedron.png) | ![Hypercube](images/retrieved/Hypercube.png) | ![Schlegel 8-cell](images/retrieved/Schlegel_wireframe_8-cell.png) |

2. **Petrie Polygon Skew Orthogonal Projections (`1-simplex_t0.svg` through `10-cube.svg`):**
   A **Petrie polygon** of a regular $n$-polytope is a skew polygon in which every $n-1$ consecutive edges (but no $n$) belong to a single facet. For the $n$-cube $\gamma_n$, the Coxeter element $c = r_0 r_1 \cdots r_{n-1} \in B_n$ has order **Coxeter number $h = 2n$**, and the projection of $\gamma_n$ onto the $2$-dimensional Coxeter plane (the eigenspace of $c$ with eigenvalues $e^{\pm \pi i / n}$) bounds the projected graph inside a regular **$2n$-gon**. Specifically, vertex $b \in \{0, 1\}^n$ projects to the complex plane $\mathbb{C} \cong \mathbb{R}^2$ via:
   $$\Pi_{\text{Petrie}}(b) = \sum_{k=0}^{n-1} \left(b_k - \frac{1}{2}\right) e^{\pi i k / n}$$

   | $n=1$: Line ($2$-gon) | $n=2$: Square ($4$-gon) | $n=3$: Cube ($6$-gon) | $n=4$: Tesseract ($8$-gon) | $n=5$: 5-Cube ($10$-gon) |
   | :---: | :---: | :---: | :---: | :---: |
   | ![1-cube](images/retrieved/1-simplex_t0.png) | ![2-cube](images/retrieved/2-cube.png) | ![3-cube](images/retrieved/3-cube_graph.png) | ![4-cube](images/retrieved/4-cube_graph.png) | ![5-cube](images/retrieved/5-cube_graph.png) |
   | **$n=6$: 6-Cube ($12$-gon)** | **$n=7$: 7-Cube ($14$-gon)** | **$n=8$: 8-Cube ($16$-gon)** | **$n=9$: 9-Cube ($18$-gon)** | **$n=10$: 10-Cube ($20$-gon)** |
   | ![6-cube](images/retrieved/6-cube_graph.png) | ![7-cube](images/retrieved/7-cube_graph.png) | ![8-cube](images/retrieved/8-cube.png) | ![9-cube](images/retrieved/9-cube.png) | ![10-cube](images/retrieved/10-cube.png) |

3. **Hypercube Graph $Q_n$, Gray Codes, and Snake-in-the-Box Codes:**
   - **Cartesian Graph Product:** $Q_n = K_2 \,\square\, K_2 \,\square\, \cdots \,\square\, K_2 = Q_{n-1} \,\square\, K_2$.
   - **Bipartite & Regular:** $Q_n$ is $n$-regular, bipartite (partitioned by Hamming weight parity $\operatorname{wt}(x) \bmod 2$), vertex-transitive, edge-transitive, and distance-transitive.
   - **Spectral Graph Theory:** The adjacency matrix $A(Q_n)$ has eigenvalues $\lambda_k = n - 2k$ with multiplicity $\binom{n}{k}$ for $k = 0, 1, \dots, n$ (and Laplacian eigenvalues $\mu_k = 2k$ with multiplicity $\binom{n}{k}$), proved via the Walsh–Hadamard eigenvectors $\chi_S(x) = (-1)^{\langle S, x \rangle}$.
   - **Hamiltonian Cycles (Reflected Binary Gray Codes):** Every $Q_n$ ($n \ge 2$) is Hamiltonian; an $n$-bit Gray code $g_i = i \oplus (i \gg 1)$ traces a Hamiltonian cycle visiting all $2^n$ vertices by flipping a single bit per step.
   - **Snake-in-the-Box & Coil-in-the-Box:** Induced paths (snakes) and induced cycles (coils) in $Q_n$ where non-adjacent path vertices maintain Hamming distance $d_H \ge 2$, used for single-error-detecting sequential state codes.

   | $Q_4$ Symmetric $K_{4,4}$ Star Projection | Inductive Construction $Q_0 \to Q_4$ | 4-Bit Gray Code on $Q_4$ | Snake & Coil in $Q_3$ |
   | :---: | :---: | :---: | :---: |
   | ![Hypercubestar](images/retrieved/Hypercubestar.png) | ![Hypercubeconstruction](images/retrieved/Hypercubeconstruction.png) | ![Gray code tesseract](images/retrieved/Gray_code_tesseract_432.png) | ![Snakes and coils](images/retrieved/Snakes_and_coils_in_the_box.png) |

---

## 3. Direct Mathematical & Systems Connection to Infinity-Context (`crates/icx-hdc`)

Infinity-Context's Plane 3 engine (`crates/icx-hdc`) directly instantiates both hypercube realizations in production Rust code to achieve `< 250 µs` associative and topological retrieval without external vector databases or GPU inference on the critical path.

### 3.1 The 10,240-Bit Boolean Hypercube ($Q_{10240} = \{0, 1\}^{10240}$, `Hypervector([u64; 160])`)

#### 3.1.1 Hardware-Aligned Representation in `crates/icx-hdc/src/hv.rs`
```rust
pub const HV_WORDS: usize = 160;
pub const HV_BITS: usize = HV_WORDS * 64; // 10,240 bits

/// 10,240-bit binary hypervector aligned to 64-byte cache lines / AVX-512 ZMM registers.
#[repr(C, align(64))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hypervector {
    pub words: [u64; HV_WORDS],
}
```
Each `Hypervector` is a single vertex $x \in \{0, 1\}^{10240}$ of the $10,240$-dimensional Boolean hypercube $Q_{10240}$:
- **Vertex Cardinality:** $|V(Q_{10240})| = 2^{10240} \approx 1.0715 \times 10^{3082}$ vertices.
- **Edge Cardinality:** $|E(Q_{10240})| = 10,240 \cdot 2^{10239} \approx 5.486 \times 10^{3085}$ edges.
- **Physical Layout:** `160` $\times$ `u64` words = `1,280` bytes = `20` $\times$ 64-byte CPU cache lines = `20` AVX-512 `zmm` (512-bit) registers.
- **Uniform Vertex Sampling (`Hypervector::from_seed`):** Seeds `(domain, symbol)` are hashed with domain separation `"ICX_HDC_10240_V1:"` via `blake3::Hasher::finalize_xof()` to fill all `1,280` bytes in `< 150 ns`. Because BLAKE3 XOF is a pseudorandom oracle, each seed is drawn uniformly and deterministically from $V(Q_{10240})$.

#### 3.1.2 Graph Geodesic = Hardware `POPCNT` Hamming Distance
On the hypercube graph $Q_{10240}$, the shortest-path edge distance between two vertices $A, B \in \{0, 1\}^{10240}$ is their exact Hamming distance:

$$d_H(A, B) = \sum_{i=0}^{10239} (A_i \oplus B_i) = \sum_{w=0}^{159} \operatorname{popcnt}\!\big(A.\text{words}[w] \oplus B.\text{words}[w]\big)$$

In `Hypervector::hamming_distance`, the loop over `0..160` compiles under LLVM to unrolled hardware `POPCNT` or AVX-512 `VPOPCNTDQ` instructions, evaluating a 10,240-dimensional geodesic in **20 SIMD instructions ($\sim 3\text{--}5\text{ ns}$)**.

#### 3.1.3 Algebraic Operators as Hyperoctahedral Automorphisms ($\operatorname{Aut}(Q_{10240}) = B_{10240}$)
In Kanerva's Binary Spatter Codes (BSC) / Holographic Reduced Representations (`HDC/HRR`), role-filler binding and sequence encoding require operations that preserve similarity (and therefore preserve graph distance on $Q_{10240}$) while mapping vectors to quasi-orthogonal regions of the hypercube:

1. **Bitwise XOR Binding (`Hypervector::bind`, $A \oplus B$) — The $(\mathbb{Z}_2)^{10240}$ Subgroup:**
   For any fixed role vector $R \in Q_{10240}$, the map $\tau_R(X) = X \oplus R$ flips the coordinates of $X$ wherever $R_i = 1$.
   - **Self-Inverse (Involution):** $(X \oplus R) \oplus R = X \oplus (R \oplus R) = X \oplus 0 = X$.
   - **Exact Graph Isometry:**
     $$d_H(A \oplus R, B \oplus R) = \operatorname{wt}\!\big((A \oplus R) \oplus (B \oplus R)\big) = \operatorname{wt}(A \oplus B) = d_H(A, B)$$
   - **Dissimilarity to Inputs:** If $R$ is an independent uniform vertex, $\mathbb{E}[d_H(X, X \oplus R)] = \operatorname{wt}(R) \approx 5,120$, so binding teleports $X$ to the equator of $X$'s Hamming sphere while preserving the exact relative geometry of any bound neighbourhood.

2. **Cyclic Word & Bit Permutation (`Hypervector::permute(shift)`, $\Pi^k$) — The $S_{10240}$ Subgroup:**
   `permute(shift)` rotates words by `shift % 160` and bits within each word by `((shift % 63) + 1)`, implementing a coordinate permutation $\Pi^k \in S_{10240} \subset B_{10240}$:
   - **Exact Graph Isometry:** Because $\Pi^k$ is a bijection on the 10,240 bit indices,
     $$d_H(\Pi^k A, \Pi^k B) = \sum_{i=0}^{10239} (\Pi^k A)_i \oplus (\Pi^k B)_i = \sum_{j=0}^{10239} A_j \oplus B_j = d_H(A, B)$$
   - **Distributivity over XOR Binding:** $\Pi^k(A \oplus B) = \Pi^k(A) \oplus \Pi^k(B)$.
   - **Non-Commutative Sequence Encoding:** In `Hypervector::encode_text`, token bigrams are encoded as $\operatorname{prev}.\text{permute}(1).\text{bind}(\operatorname{curr}) = \Pi^1(H_{t-1}) \oplus H_t$, which is non-commutative ($\Pi^1(A) \oplus B \ne \Pi^1(B) \oplus A$) and prevents anagram collisions.

#### 3.1.4 Concentration of Measure & Quasi-Orthogonality on $Q_{10240}$
For two independent uniformly sampled vertices $A, B \in Q_D$ ($D = 10,240$), each bit difference $X_i = A_i \oplus B_i \sim \operatorname{Bernoulli}(1/2)$ is independent, so $d_H(A, B) \sim \operatorname{Binomial}(D, 1/2)$:
- **Expected Equator Distance:** $\mu = \frac{D}{2} = 5,120\text{ bits}$.
- **Variance & Standard Deviation:** $\sigma^2 = \frac{D}{4} = 2,560$, so $\sigma = \sqrt{2,560} = 16\sqrt{10} \approx 50.59644\text{ bits}$ (only **$0.4941\%$** of $D$).
- **Hoeffding Tail Bound:** For any normalized similarity deviation $\epsilon = \frac{5120 - d_H}{5120} \in (0, 1]$, the probability that two unrelated seeds have similarity $\operatorname{sim}(A, B) \ge \epsilon$ (i.e., $d_H(A, B) \le 5120(1 - \epsilon) = D(\frac{1}{2} - \frac{\epsilon}{2})$) is bounded by:
  $$\mathbb{P}\!\big(\operatorname{sim}(A, B) \ge \epsilon\big) = \mathbb{P}\!\left(d_H(A, B) - \frac{D}{2} \le -\frac{\epsilon D}{2}\right) \le \exp\!\left(-\frac{2 (\epsilon D / 2)^2}{D}\right) = \exp\!\left(-\frac{D \epsilon^2}{2}\right)$$

| Normalized Similarity Threshold $\epsilon$ | Maximum Hamming Distance $d_H$ | Z-Score ($z = \frac{5120 - d_H}{\sigma}$) | False-Positive Collision Probability $\le \exp(-5120 \epsilon^2)$ |
| :---: | :---: | :---: | :---: |
| $\epsilon = 0.05$ (`hv.rs` test gate) | $4,864\text{ bits}$ | $5.06\,\sigma$ | $\le e^{-12.8} \approx 2.76 \times 10^{-6}$ (exact binomial $\approx 2.09 \times 10^{-7}$) |
| $\epsilon = 0.10$ | $4,608\text{ bits}$ | $10.12\,\sigma$ | $\le e^{-51.2} \approx 5.81 \times 10^{-23}$ |
| $\epsilon = 0.25$ (`hv.rs` bundler gate) | $3,840\text{ bits}$ | $25.30\,\sigma$ | $\le e^{-320} \approx 9.56 \times 10^{-140}$ |

This exponential concentration of measure around the equator $d_H = 5,120$ is why `Hypervector::similarity` maps the upper hemisphere $d_H \in [0, 5120]$ linearly onto $[0.0, 1.0]$:
$$\operatorname{sim}(A, B) = 1.0 - \frac{\min(d_H(A, B), 5120)}{5120} \in [0.0, 1.0]$$

#### 3.1.5 Infini-Attention Delta-Rule Majority Bundling (`HdcBundler`)
While standard HDC superposition sums bipolar vectors in $\{-1, +1\}^{10240}$ (`acc: [i16; 10240]`) and projects back onto a vertex of $Q_{10240}$ via coordinate-wise majority thresholding ($\operatorname{acc}[b] \ge 0 \implies 1$, else $0$), naive summation allows a single boilerplate log message repeated $50\times$ to dominate the hypercube coordinate signs and erase rare anomalies.

`HdcBundler` in `crates/icx-hdc/src/hv.rs` solves this geometrically on $Q_{10240}$ using the **Infini-attention Delta Rule** (`arXiv:2404.07143`): before adding a vector $V$ to the bundle, it measures the current readout vertex $M$'s similarity $\operatorname{sim}(M, V)$. If $\operatorname{sim}(M, V) \ge 0.85$ ($d_H(M, V) \le 768$), the vector is already inside the bundle's spherical cap on $Q_{10240}$ and receives weight $0$; otherwise it receives weight $w = \max(1, \operatorname{round}(16 \cdot (1 - \operatorname{sim}(M, V))))$.

---

### 3.2 The 16-Dimensional Continuous Hypercube ($[-1, +1]^{16}$, `FractalCoord`)

While $Q_{10240}$ indexes *unordered/bigram bag-of-bindings content*, agent execution also exhibits multi-step **temporal trajectories** (e.g., an edit $\to$ compile error $\to$ test panic $\to$ retry loop). Comparing $L$-step suffixes across $N$ historical states naively costs $O(N \cdot L)$.

Following Gusev (*Fractal KV-Cache Archives*, `arXiv:2607.07144`), `crates/icx-hdc/src/fractal.rs` embeds every agent state trajectory as a single point inside the **16-dimensional origin-centered continuous hypercube $[-1, +1]^{16}$**.

#### 3.2.1 Block-Majority Projection $\pi_{10240 \to 16}: Q_{10240} \to \{-1, +1\}^{16}$
In `FractalCoord::corner_vertex(hv: &Hypervector) -> [f64; 16]`, the 160 `u64` words of a `Hypervector` are partitioned into $16$ contiguous blocks of $10$ words ($640\text{ bits}$ per block). Each block is majority-thresholded at $320$ ones to select a bipolar coordinate $v_j(H) \in \{-1.0, +1.0\}$:

$$v_j(H) = \begin{cases} +1.0 & \text{if } \sum_{w=10j}^{10j+9} \operatorname{popcnt}(H.\text{words}[w]) \ge 320 \\ -1.0 & \text{otherwise} \end{cases} \qquad (j \in 0..16)$$

Thus $v(H) \in \{-1, +1\}^{16}$ is one of the $2^{16} = 65,536$ extreme corner vertices of the 16-cube $[-1, +1]^{16}$.

#### 3.2.2 Contractive Iterated Function System (IFS) & Nested Sub-Hypercubes
Starting from the origin $z_0 = (0, 0, \dots, 0) \in [-1, +1]^{16}$, each observed event $s_t$ applies the contractive similarity map $f_{s_t}(z) = \frac{1}{2} z + \frac{1}{2} v(s_t)$ with contraction ratio $c = \frac{1}{2}$:

$$z_t = \frac{1}{2} z_{t-1} + \frac{1}{2} v(s_t) = \sum_{i=0}^{t-1} 2^{-(i+1)} v(s_{t-i}) \in [-1, +1]^{16}$$

Geometrically:
- After **1 step** ($s_t$), the sign of each coordinate $z_{t,j} \in [-1, 0)$ vs. $[0, +1]$ determines which of the $2^{16} = 65,536$ **first-level orthants (sub-16-cubes of side length $1$)** contains $z_t$. This 16-bit orthant mask is extracted by `FractalCoord::quadrant_signature_u16() -> u16` and encoded into the AHTL `@o1:o2` Fractal Trajectory Coordinate and Braille-256 Channel B (`B_4, B_5`).
- After **$k$ steps** ($s_{t-k+1}, \dots, s_t$), the trailing $k$ transitions localize $z_t$ inside a **dyadic sub-16-cube of side length $2^{1-k}$** (and Chebyshev radius $2^{-k}$), while all older history ($i \ge k$) is compressed by $2^{-k}$ into the position of $z_t$ *within* that sub-16-cube.

#### 3.2.3 Proof of the $O(1)$ Chebyshev ($L_\infty$) Suffix Theorem on $[-1, +1]^{16}$
Why does `FractalCoord::chebyshev_distance` use the **Chebyshev ($L_\infty$) metric** rather than the Euclidean ($L_2$) metric?
Because the unit ball of $(\mathbb{R}^{16}, \|\cdot\|_\infty)$ **is the 16-cube $[-1, +1]^{16}$ itself**!

**Theorem (Contractive Hypercube Suffix Theorem):**
Let $z_a, z_b \in [-1, +1]^{16}$ be the `FractalCoord` states of two trajectories that share the exact same trailing $k$ state corner vertices:
$$v(s_{a, t_a - i}) = v(s_{b, t_b - i}) \quad \text{for all } i = 0, 1, \dots, k-1$$
and differ at step $i = k$ in at least one coordinate $j^* \in \{0, \dots, 15\}$ (so $|v_{j^*}(s_{a, t_a - k}) - v_{j^*}(s_{b, t_b - k})| = 2$). Then:
1. All $i < k$ terms in $z_a - z_b$ cancel identically:
   $$z_a - z_b = \sum_{i=k}^{\infty} 2^{-(i+1)} \big(v(s_{a, t_a - i}) - v(s_{b, t_b - i})\big)$$
2. Since $|v_j(\cdot) - v_j(\cdot)| \le 2$ for every coordinate $j \in 0..16$, the Chebyshev ($L_\infty$) distance is bounded above by the geometric series:
   $$d_\infty(z_a, z_b) = \|z_a - z_b\|_\infty \le \sum_{i=k}^{\infty} 2^{-(i+1)} \cdot 2 = \sum_{i=k}^{\infty} 2^{-i} = 2^{1-k}$$
   Dividing by the hypercube diameter $2$ gives the normalized Chebyshev distance $d_{\text{norm}} = \frac{1}{2} d_\infty(z_a, z_b) \le 2^{-k}$.
3. Taking $-\log_2(d_{\text{norm}})$ yields the $O(1)$ upper-bound suffix-depth proximity heuristic (`FractalCoord::shared_suffix_depth`; when exact suffix matching is required, callers also check orthant sign equality `quadrant_signature_u16()` or trailing state IDs):
   $$L_{\text{shared}}(z_a, z_b) = \min\!\Big(\big\lfloor -\log_2\!\big(\tfrac{1}{2} \|z_a - z_b\|_\infty\big) \big\rfloor, \; \text{depth}_a, \; \text{depth}_b, \; 32\Big)$$

In `crates/icx-hdc/src/lib.rs`, `HdcAssociativeIndex::query_associative` fuses the $Q_{10240}$ Hamming similarity, the $[-1, +1]^{16}$ Chebyshev suffix depth, and the Titans Surprise momentum score into a unified retrieval rank:

$$\text{score}(q, e) = 0.65 \cdot \operatorname{sim}_{Q_{10240}}(q, e) + 0.20 \cdot S_{\text{Titans}}(e) + 0.15 \cdot \operatorname{clamp}\!\left(\frac{L_{\text{shared}}(z_{\text{curr}}, z_e)}{8.0},\, 0.0,\, 1.0\right)$$

---

## 4. Complete Visual Archive in `images/`

All diagrams are organized into [`images/generated/`](images/generated/) (first-party Infinity-Context synthesis diagrams) and [`images/retrieved/`](images/retrieved/) (diagrams archived from `https://en.wikipedia.org/wiki/Hypercube` and `https://en.wikipedia.org/wiki/Hypercube_graph`) in both original vector (`.svg`) and high-resolution rasterized (`.png` / `.gif`) formats:

### 4.1 First-Party Infinity-Context Synthesis Diagrams (`images/generated/`)
- [`images/generated/0d_to_4d_hypercube_progression.svg`](images/generated/0d_to_4d_hypercube_progression.svg) / [`.png`](images/generated/0d_to_4d_hypercube_progression.png) — Minkowski sum progression of $n$-cubes ($\gamma_0$ point to $\gamma_4$ tesseract) with binary coordinates and $f$-vectors.
- [`images/generated/icx_hdc_10240_hypercube_and_fcga.svg`](images/generated/icx_hdc_10240_hypercube_and_fcga.svg) / [`.png`](images/generated/icx_hdc_10240_hypercube_and_fcga.png) — Dual hypercube architecture in `crates/icx-hdc`: $Q_{10240}$ (`Hypervector`) and $[-1, +1]^{16}$ (`FractalCoord`).

### 4.2 0D–4D Construction, Perspective, and Schlegel Projections (`images/retrieved/`)
- [`images/retrieved/Dimension_levels.svg`](images/retrieved/Dimension_levels.svg) / [`.png`](images/retrieved/Dimension_levels.png) — 0D point, 1D line segment, 2D square, 3D cube, 4D tesseract progression.
- [`images/retrieved/From_Point_to_Tesseract_(Looped_Version).gif`](images/retrieved/From_Point_to_Tesseract_(Looped_Version).gif) — Animated Minkowski sweep from $0$-cube to $4$-cube.
- [`images/retrieved/Hexahedron.svg`](images/retrieved/Hexahedron.svg) / [`.png`](images/retrieved/Hexahedron.png) — Perspective projection of the $3$-cube (hexahedron).
- [`images/retrieved/Hypercube.svg`](images/retrieved/Hypercube.svg) / [`.png`](images/retrieved/Hypercube.png) — Perspective Schlegel-style projection of the $4$-cube (tesseract).
- [`images/retrieved/Schlegel_wireframe_8-cell.png`](images/retrieved/Schlegel_wireframe_8-cell.png) — 3D wireframe Schlegel projection of the $8$-cell (tesseract).
- [`images/retrieved/8-cell.gif`](images/retrieved/8-cell.gif) & [`images/retrieved/Tesseract.gif`](images/retrieved/Tesseract.gif) — Animated 4D rotations of the unit tesseract.

### 4.3 Petrie Polygon Orthographic Projections ($1$-Cube through $10$-Cube, `images/retrieved/`)
- [`images/retrieved/1-simplex_t0.svg`](images/retrieved/1-simplex_t0.svg) / [`.png`](images/retrieved/1-simplex_t0.png) — $1$-cube (line segment, $2$ vertices, $1$ edge).
- [`images/retrieved/2-cube.svg`](images/retrieved/2-cube.svg) / [`.png`](images/retrieved/2-cube.png) — $2$-cube (square, $4$ vertices, $4$ edges).
- [`images/retrieved/3-cube_graph.svg`](images/retrieved/3-cube_graph.svg) / [`.png`](images/retrieved/3-cube_graph.png) — $3$-cube (cube $Q_3$, $8$ vertices, $12$ edges, hexagonal Petrie perimeter).
- [`images/retrieved/4-cube_graph.svg`](images/retrieved/4-cube_graph.svg) / [`.png`](images/retrieved/4-cube_graph.png) — $4$-cube (tesseract $Q_4$, $16$ vertices, $32$ edges, octagonal Petrie perimeter).
- [`images/retrieved/5-cube_graph.svg`](images/retrieved/5-cube_graph.svg) / [`.png`](images/retrieved/5-cube_graph.png) — $5$-cube (penteract $Q_5$, $32$ vertices, $80$ edges, decagonal Petrie perimeter).
- [`images/retrieved/6-cube_graph.svg`](images/retrieved/6-cube_graph.svg) / [`.png`](images/retrieved/6-cube_graph.png) — $6$-cube (hexeract $Q_6$, $64$ vertices, $192$ edges, $12$-gonal Petrie perimeter).
- [`images/retrieved/7-cube_graph.svg`](images/retrieved/7-cube_graph.svg) / [`.png`](images/retrieved/7-cube_graph.png) — $7$-cube (hepteract $Q_7$, $128$ vertices, $448$ edges, $14$-gonal Petrie perimeter).
- [`images/retrieved/8-cube.svg`](images/retrieved/8-cube.svg) / [`.png`](images/retrieved/8-cube.png) — $8$-cube (octeract $Q_8$, $256$ vertices, $1,024$ edges, $16$-gonal Petrie perimeter).
- [`images/retrieved/9-cube.svg`](images/retrieved/9-cube.svg) / [`.png`](images/retrieved/9-cube.png) — $9$-cube (enneract $Q_9$, $512$ vertices, $2,304$ edges, $18$-gonal Petrie perimeter).
- [`images/retrieved/10-cube.svg`](images/retrieved/10-cube.svg) / [`.png`](images/retrieved/10-cube.png) — $10$-cube (dekeract $Q_{10}$, $1,024$ vertices, $5,120$ edges, $20$-gonal Petrie perimeter).

### 4.4 Hypercube Graphs ($Q_n$), Gray Codes, and Generalized Complex Hypercubes ($\gamma_n^p$, `images/retrieved/`)
- [`images/retrieved/Hypercubestar.svg`](images/retrieved/Hypercubestar.svg) / [`.png`](images/retrieved/Hypercubestar.png) — Symmetric 4-regular drawing of $Q_4$.
- [`images/retrieved/Hypercubeconstruction.png`](images/retrieved/Hypercubeconstruction.png) — Inductive Cartesian product construction of $Q_1, Q_2, Q_3, Q_4$.
- [`images/retrieved/Gray_code_tesseract_432.svg`](images/retrieved/Gray_code_tesseract_432.svg) / [`.png`](images/retrieved/Gray_code_tesseract_432.png) — Hamiltonian Reflected Binary Gray Code cycle on $Q_4$.
- [`images/retrieved/Snakes_and_coils_in_the_box.svg`](images/retrieved/Snakes_and_coils_in_the_box.svg) / [`.png`](images/retrieved/Snakes_and_coils_in_the_box.png) — Snake-in-the-box and coil-in-the-box induced error-detecting subgraphs on $Q_3$.
- Generalized complex hypercube projections ($\gamma_n^p$):
  - $p=3$ ($\mathbb{C}^2 \dots \mathbb{C}^4$): [`images/retrieved/3-generalized-2-cube_skew.svg`](images/retrieved/3-generalized-2-cube_skew.svg) ([`.png`](images/retrieved/3-generalized-2-cube_skew.png)), [`images/retrieved/3-generalized-3-cube.svg`](images/retrieved/3-generalized-3-cube.svg) ([`.png`](images/retrieved/3-generalized-3-cube.png)), [`images/retrieved/3-generalized-4-cube.svg`](images/retrieved/3-generalized-4-cube.svg) ([`.png`](images/retrieved/3-generalized-4-cube.png)).
  - $p=4$ ($\mathbb{C}^2 \dots \mathbb{C}^4$): [`images/retrieved/4-generalized-2-cube.svg`](images/retrieved/4-generalized-2-cube.svg) ([`.png`](images/retrieved/4-generalized-2-cube.png)), [`images/retrieved/4-generalized-3-cube.svg`](images/retrieved/4-generalized-3-cube.svg) ([`.png`](images/retrieved/4-generalized-3-cube.png)), [`images/retrieved/4-generalized-4-cube.svg`](images/retrieved/4-generalized-4-cube.svg) ([`.png`](images/retrieved/4-generalized-4-cube.png)).

---

## 5. Complete Bibliography & External References Linked from Wikipedia

### 5.1 Academic Literature & Monographs Cited in `Hypercube` and `Hypercube_graph`
1. **Coxeter, H. S. M.** (1973). *Regular Polytopes* (3rd ed.). New York: Dover Publications. §7.2 (pp. 122–123, Fig. 7.2c) & Table I(iii) (p. 296). ISBN `0-486-61480-8`. Archive.org: <https://archive.org/details/regularpolytopes0000coxe/page/122>
2. **Coxeter, H. S. M.** (1974). *Regular Complex Polytopes*. London & New York: Cambridge University Press, p. 180. MathSciNet: [MR0370328](https://mathscinet.ams.org/mathscinet-getitem?mr=0370328)
3. **Elte, E. L.** (1912). *The Semiregular Polytopes of the Hyperspaces*. Groningen: University of Groningen, Chapter IV ("Five dimensional semiregular polytope"). ISBN `141817968X`.
4. **Johnson, Norman W.** (2018). *Geometries and Transformations*. Cambridge University Press, p. 224.
5. **Hill, Frederick J.; Peterson, Gerald R.** (1974). *Introduction to Switching Theory and Logical Design* (2nd ed.). New York: John Wiley & Sons, Chapter 7.1 ("Cubical Representation of Boolean Functions" — Gray codes, Veitch diagrams, and Karnaugh maps on hypercubes). ISBN `0-471-39882-9`.
6. **Dooren, Paul; Ridder, Luc** (1976). "An adaptive algorithm for numerical integration over an $n$-dimensional cube". *Journal of Computational and Applied Mathematics*, 2(3): 207–217. DOI: <https://doi.org/10.1016/0771-050X(76)90005-X>
7. **Yang, Xiaofan; Tang, Yuan** (2007). "A $(4n - 9)/3$ diagnosis algorithm on $n$-dimensional cube network". *Information Sciences*, 177(8): 1771–1781. DOI: <https://doi.org/10.1016/j.ins.2006.10.002> (<https://www.sciencedirect.com/science/article/pii/S0020025506003173>)
8. **Vořechovský, Miroslav; Mašek, Jan; Eliáš, Jan** (2019). "Distance-based optimal sampling in a hypercube: Analogies to N-body systems". *Advances in Engineering Software*, 137: 102709. ISSN [0965-9978](https://search.worldcat.org/issn/0965-9978). DOI: <https://doi.org/10.1016/j.advengsoft.2019.102709>
9. **Alon, Noga** (1992). "Transmitting in the $n$-dimensional cube". *Discrete Applied Mathematics*, 37–38: 9–11. DOI: <https://doi.org/10.1016/0166-218X(92)90121-P>
10. **Bowen, J. P.** (April 1982). "Hypercube". *Practical Computing*, 5(4): 97–99. Archived URL: <https://web.archive.org/web/20080630081518/http://www.jpbowen.com/publications/ndcubes.html>
11. **Harary, Frank; Hayes, John P.; Wu, Horng-Jyh** (1988). "A survey of the theory of hypercube graphs". *Computers & Mathematics with Applications*, 15(4): 277–289. DOI: <https://doi.org/10.1016/0898-1221(88)90213-1> / DeepBlue PDF: <http://deepblue.lib.umich.edu/bitstream/handle/2027.42/27522/0000566.pdf?sequence=1>

### 5.2 Hyperdimensional Computing & Fractal IFS References Used in `crates/icx-hdc`
12. **Kanerva, Pentti** (2009). "Hyperdimensional Computing: An Introduction to Computing in Distributed Representation with High-Dimensional Random Vectors". *Cognitive Computation*, 1(2): 139–159.
13. **Plate, Tony A.** (1995). "Holographic Reduced Representations". *IEEE Transactions on Neural Networks*, 6(3): 623–641.
14. **Munkhdalai, Tsendsuren; Faruqui, Manaal; Gopal, Siddharth** (2024). "Leave No Context Behind: Efficient Infinite Context Transformers with Infini-attention". `arXiv:2404.07143` ([local PDF](https://arxiv.org/abs/2404.07143)).
15. **Gusev, Ilya** (2026). "Fractal KV-Cache Archives via Contractive Iterated Function Systems". `arXiv:2607.07144` ([local PDF](../papers/2607.07144-Fractal-KV-Cache-Archives.pdf)).
16. **Behrouz, Ali; Zhong, Peilin; Mirrokni, Vahab** (2025). "Titans: Learning to Memorize at Test Time". `arXiv:2501.00663` ([local PDF](../papers/2501.00663-Titans-Learning-to-Memorize-at-Test-Time.pdf)).

### 5.3 External Mathematical Databases & Demonstrations
- **Wolfram MathWorld — Hypercube:** <https://mathworld.wolfram.com/Hypercube.html>
- **Wolfram MathWorld — Hypercube Graph:** <https://mathworld.wolfram.com/HypercubeGraph.html>
- **Wolfram Demonstrations Project — Rotating a Hypercube (Enrique Zeleny):** <http://demonstrations.wolfram.com/RotatingAHypercube/>
- **OEIS A001787 — Number of edges in an $n$-dimensional hypercube ($n 2^{n-1}$):** <https://oeis.org/A001787>
- **OEIS A038207 — Triangle of $k$-faces $E_{k,n} = \binom{n}{k} 2^{n-k}$ of an $n$-cube:** <https://oeis.org/A038207>
- **Rudy Rucker & Farideh Dormishian's Hypercube Downloads (Wayback Archive):** <https://web.archive.org/web/20130326090312/http://www.cs.sjsu.edu/~rucker/hypercube.htm>
