#!/usr/bin/env python3
"""Independent dense three-node PageRank vector, no production imports."""
import json

# Directed facts A->B weight1, B->C weight2, A->C weight3.
# Implementation adds half-weight reverse arcs, yielding these normalized rows.
transition = [[0, .25, .75], [.2, 0, .8], [.6, .4, 0]]
rank = [1., 0., 0.]
results = []
for step in range(1, 7):
    rank = [(.15 if j == 0 else 0) + .85 * sum(rank[i] * transition[i][j] for i in range(3))
            for j in range(3)]
    assert abs(sum(rank) - 1) < 1e-12
    results.append({'iterations': step, 'A': rank[0], 'B': rank[1], 'C': rank[2]})
print(json.dumps(results, indent=2))
