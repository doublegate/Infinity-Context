// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later
// MathJax configuration for the documentation site (Zensical; pymdownx.arithmatex generic mode).
window.MathJax = {
  tex: {
    inlineMath: [["\\(", "\\)"]],
    displayMath: [["\\[", "\\]"]],
    processEscapes: true,
    processEnvironments: true,
  },
  options: {
    ignoreHtmlClass: ".*|",
    processHtmlClass: "arithmatex",
  },
};

// Re-typeset after each instant-navigation page change.
// The first emission can come before MathJax has finished starting up; startup typesets that
// page itself.
document$.subscribe(() => {
  if (!window.MathJax?.startup?.output || !window.MathJax.typesetPromise) return;
  MathJax.startup.output.clearCache();
  MathJax.typesetClear();
  MathJax.texReset();
  MathJax.typesetPromise();
});
