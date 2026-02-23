# Appendix A: Worked Example

This document traces the complete PKI example through every phase of the
`mdbook-objgraph` pipeline, from raw input syntax to final layout coordinates.
It serves as a verification reference: an implementor should be able to
reproduce each intermediate result exactly by following the algorithms described
in the sibling design documents.

---

This appendix traces the complete example from section 3.3 through every
pipeline phase to produce concrete intermediate data structures. An implementor
should be able to reproduce these results.

### A.1 Input

> Input syntax: see [SYNTAX.md](SYNTAX.md)

```obgraph
domain "PKI" {
  node ca "Certificate Authority" @anchored @selected {
    subject.common_name    @constrained
    subject.org            @constrained
    public_key             @constrained
  }

  node cert "Certificate" {
    issuer.common_name     @critical
    issuer.org             @critical
    subject.common_name
    subject.org
    public_key             @constrained
    signature              @critical
  }
}

domain "Transport" {
  node tls "TLS Session" {
    server_cert            @critical
    cipher_suite
  }
}

node revocation "Revocation List" @anchored {
  crl                      @constrained
}

cert <- ca : sign
tls <- cert

cert::issuer.common_name <= ca::subject.common_name
cert::issuer.org <= ca::subject.org
cert::signature <= ca::public_key : verified_by
tls::server_cert <= cert::public_key
cert::subject.common_name <= revocation::crl : not_in
```

### A.2 Parse Result (AST)

> AST types: see [SYNTAX.md](SYNTAX.md)

```
Domains:
  Domain("PKI", members: [ca, cert])
  Domain("Transport", members: [tls])

Nodes:
  ca   display="Certificate Authority"  @anchored @selected  domain=PKI
    subject.common_name    @constrained
    subject.org            @constrained
    public_key             @constrained

  cert display="Certificate"            domain=PKI
    issuer.common_name     @critical
    issuer.org             @critical
    subject.common_name    (no annotations)
    subject.org            (no annotations)
    public_key             @constrained
    signature              @critical

  tls  display="TLS Session"            domain=Transport
    server_cert            @critical
    cipher_suite           (no annotations)

  revocation display="Revocation List"  @anchored  domain=none
    crl                    @constrained

Anchors:
  cert <- ca : sign
  tls  <- cert

Constraints:
  cert::issuer.common_name <= ca::subject.common_name         (equality)
  cert::issuer.org         <= ca::subject.org                 (equality)
  cert::signature          <= ca::public_key : verified_by
  tls::server_cert         <= cert::public_key                (equality)
  cert::subject.common_name <= revocation::crl : not_in

Derivations: (none in this example)
```

### A.3 Graph Construction

> Data structures: see [GRAPH_MODEL.md](GRAPH_MODEL.md) §2.5

Property ID assignment (global, sequential):

| PropId | Node       | Name                | critical | constrained |
| ------ | ---------- | ------------------- | -------- | ----------- |
| P0     | ca         | subject.common_name | false    | true        |
| P1     | ca         | subject.org         | false    | true        |
| P2     | ca         | public_key          | false    | true        |
| P3     | cert       | issuer.common_name  | true     | false       |
| P4     | cert       | issuer.org          | true     | false       |
| P5     | cert       | subject.common_name | false    | false       |
| P6     | cert       | subject.org         | false    | false       |
| P7     | cert       | public_key          | false    | true        |
| P8     | cert       | signature           | true     | false       |
| P9     | tls        | server_cert         | true     | false       |
| P10    | tls        | cipher_suite        | false    | false       |
| P11    | revocation | crl                 | false    | true        |

Edges:

| EdgeId | Type       | Source                        | Target                         | Operation   |
| ------ | ---------- | ----------------------------- | ------------------------------ | ----------- |
| E0     | Anchor     | ca (parent)                   | cert (child)                   | sign        |
| E1     | Anchor     | cert (parent)                 | tls (child)                    | —           |
| E2     | Constraint | P0 (ca::subject.common_name)  | P3 (cert::issuer.common_name)  | equality    |
| E3     | Constraint | P1 (ca::subject.org)          | P4 (cert::issuer.org)          | equality    |
| E4     | Constraint | P2 (ca::public_key)           | P8 (cert::signature)           | verified_by |
| E5     | Constraint | P7 (cert::public_key)         | P9 (tls::server_cert)          | equality    |
| E6     | Constraint | P11 (revocation::crl)         | P5 (cert::subject.common_name) | not_in      |

Note: In constraint edges, "source" is the right-hand side (trust source) and
"target" is the left-hand side (destination being verified). Trust flows source
→ target.

Port-level adjacency (prop_edges):

```
P0  → [E2]          (outgoing constraint)
P1  → [E3]          (outgoing constraint)
P2  → [E4]          (outgoing constraint)
P3  → [E2]          (incoming constraint)
P4  → [E3]          (incoming constraint)
P5  → [E6]          (incoming constraint)
P7  → [E5]          (outgoing constraint)
P8  → [E4]          (incoming constraint)
P9  → [E5]          (incoming constraint)
P11 → [E6]          (outgoing constraint)
```

Node-level adjacency (anchor edges):

```text
ca:         node_children → [E0]
cert:       node_parent → E0,  node_children → [E1]
tls:        node_parent → E1
revocation: (no anchor edges)
```

### A.4 Validation

> Validation rules: see [OVERVIEW.md](OVERVIEW.md) §7.2

Checks performed (all pass for this input):

1. No duplicate node identifiers. ✓
2. No duplicate property names within any node. ✓
3. All node/property references resolve. ✓
4. No `@constrained` property is a constraint destination. ✓ (P0, P1, P2, P7,
   P11 are only sources)
5. `@anchored` nodes (ca, revocation) have no incoming anchors. ✓
6. No node has multiple incoming anchors. ✓
7. No nullary derivations. ✓ (no derivations)
8. Topological sort succeeds → no cycles. ✓

### A.5 State Propagation

> Algorithm: see [GRAPH_MODEL.md](GRAPH_MODEL.md) §2.6

**Initial state** (from annotations only):

| Element                      | anchored | constrained_eff | Reason             |
| ---------------------------- | -------- | --------------- | ------------------ |
| ca (node)                    | true     | —               | @anchored              |
| P0 (ca::subject.common_name) | —        | true            | @constrained       |
| P1 (ca::subject.org)         | —        | true            | @constrained       |
| P2 (ca::public_key)          | —        | true            | @constrained       |
| P7 (cert::public_key)        | —        | true            | @constrained       |
| revocation (node)            | true     | —               | @anchored              |
| P11 (revocation::crl)        | —        | true            | @constrained       |
| cert, tls (nodes)            | false    | —               | no @anchored           |
| all other properties         | —        | false           | no @constrained    |

`verified(ca)` = true (no @critical props — vacuously true)
`verified(revocation)` = true (no @critical props — vacuously true)
`verified(cert)` = false (P3, P4, P8 not yet constrained_eff)
`verified(tls)` = false (P9 not yet constrained_eff)

**Initial node_worklist: {ca, revocation}**
**Initial prop_worklist: {P0, P1, P2, P7, P11}**

**Property phase 1** (drain prop_worklist):

- **P0**: ca is anchored+verified → E2: P3 constrained_eff = true, push P3.
- **P1**: ca is anchored+verified → E3: P4 constrained_eff = true, push P4.
- **P2**: ca is anchored+verified → E4: P8 constrained_eff = true, push P8.
- **P7**: cert is NOT anchored → skip propagation.
- **P11**: revocation is anchored+verified → E6: P5 constrained_eff = true, push P5.
- **P3**: cert is NOT anchored → skip. (verified(cert) still needs anchored)
- **P4**: cert is NOT anchored → skip.
- **P8**: cert is NOT anchored → skip.
- **P5**: cert is NOT anchored → skip.

prop_worklist empty.

**Node phase 1** (drain node_worklist):

- **ca**: anchored+verified → process children
  - E0: anchor cert. anchored[cert] = true.
    - Push P7 (cert's @constrained prop) to prop_worklist.
    - verified(cert) = P3 ✓, P4 ✓, P8 ✓ → true! Push cert to node_worklist.
- **revocation**: anchored+verified → no children.

node_worklist: {cert}

**Property phase 2**:

- **P7** (cert::public_key): cert is anchored+verified → E5: P9 constrained_eff = true, push P9.
  - verified(cert) still true → push cert to node_worklist (will be a no-op).

prop_worklist: {P9}

- **P9** (tls::server_cert): tls is anchored? No, not yet → skip.
  - verified(tls) = false → don't push tls.

prop_worklist empty.

**Node phase 2**:

- **cert** (from push during P7 processing): anchored+verified → process children
  - E1: anchor tls. anchored[tls] = true.
    - tls has no @constrained props → nothing to push.
    - verified(tls) = P9 constrained_eff? Yes! → true. Push tls to node_worklist.
- **cert** (second time, from property phase): anchored+verified → E1: tls already anchored, skip.

node_worklist: {tls}

**Property phase 3**: prop_worklist empty — skip.

**Node phase 3**:

- **tls**: anchored+verified → no children.

node_worklist empty. **Done.**

**Final state:**

| Element           | anchored | constrained_eff | verified | Note                               |
| ----------------- | -------- | --------------- | -------- | ---------------------------------- |
| ca                | true     | —               | true     | @anchored; no critical props           |
| P0, P1, P2        | —        | true            | —        | @constrained annotation            |
| revocation        | true     | —               | true     | @anchored; no critical props           |
| P11               | —        | true            | —        | @constrained annotation            |
| cert              | true     | —               | true     | anchored by ca; P3, P4, P8 all ✓   |
| P3                | —        | true            | —        | constrained by P0 via E2           |
| P4                | —        | true            | —        | constrained by P1 via E3           |
| P5                | —        | true            | —        | constrained by P11 via E6          |
| P6                | —        | false           | —        | informational; no constraint       |
| P7                | —        | true            | —        | @constrained annotation            |
| P8                | —        | true            | —        | constrained by P2 via E4           |
| tls               | true     | —               | true     | anchored by cert; P9 ✓             |
| P9                | —        | true            | —        | constrained by P7 via E5           |
| P10               | —        | false           | —        | informational; no constraint       |

All nodes are anchored and verified. P6 and P10 are informational (not @critical)
and unconstrained — this is not a problem since they don't participate in any
node's `verified` computation.

### A.6 Layer Assignment

> Algorithm: see [LAYOUT.md](LAYOUT.md) §4.2.2

All edges flow from trust sources (right-hand side) to destinations (left-hand
side). For layout, edges point downward: parents/sources at top,
children/destinations at bottom.

**Topological order** (sources before targets): ca, revocation → cert → tls

Since there are no derivations, all elements go on even layers. Minimum span for
node-to-node edges is 2.

**Network simplex result:**

| Layer    | Type       | Elements            |
| -------- | ---------- | ------------------- |
| 0 (even) | Node       | ca, revocation      |
| 1 (odd)  | Derivation | (empty — collapses) |
| 2 (even) | Node       | cert                |
| 3 (odd)  | Derivation | (empty — collapses) |
| 4 (even) | Node       | tls                 |

**Edge spans:**

| Edge                    | Span | Weight | Weighted span |
| ----------------------- | ---- | ------ | ------------- |
| E0 (Anchor: ca→cert)    | 2    | 3      | 6             |
| E1 (Anchor: cert→tls)   | 2    | 3      | 6             |
| E2 (Constraint: P0→P3)  | 2    | 1      | 2             |
| E3 (Constraint: P1→P4)  | 2    | 1      | 2             |
| E4 (Constraint: P2→P8)  | 2    | 1      | 2             |
| E5 (Constraint: P7→P9)  | 2    | 1      | 2             |
| E6 (Constraint: P11→P5) | 2    | 1      | 2             |

Total weighted edge length: 22. This is optimal — all edges have minimum
possible span.

### A.7 Crossing Minimization

> Algorithm: see [LAYOUT.md](LAYOUT.md) §4.2.3

Layer 0 has two nodes: ca and revocation. Layer 2 has one node: cert. Layer 4
has one node: tls.

With only one node in layers 2 and 4, the only freedom is the ordering of ca and
revocation in layer 0.

**Ordering ca left, revocation right:**

- E0 (ca→cert): ca is at position 0, cert at position 0. No conflict.
- E5 (revocation::crl→cert::subject.common_name): revocation is at position 1,
  cert at position 0. Routes right-to-left.
- E2 (ca::subject.common_name→cert::issuer.common_name): nearly aligned
  horizontally. Near-vertical.
- Constraint edges from ca properties to cert properties all go left-to-right
  within the column (or straight down). No crossings with E5.
- Crossings: 0 with this ordering.

**Property ordering within cert:**

The barycenter heuristic for cert's properties (all edges come from layer 0):

- P3 (issuer.common_name): connected to P0 on ca → barycenter from ca's position
  (left)
- P4 (issuer.org): connected to P1 on ca → barycenter from ca's position (left)
- P8 (signature): connected to P2 on ca → barycenter from ca's position (left)
- P5 (subject.common_name): connected to P11 on revocation → barycenter from
  revocation's position (right)

Properties connected to ca should be ordered above those connected to
revocation. Properties P7 (public_key), P6 (subject.org) have no incoming edges
— they keep their declared order.

Optimal property order in cert: P3, P4, P8, P7, P5, P6 (ca-connected first, then
unconnected, then revocation-connected).

But since property reordering is a quality optimization, the initial
implementation may keep declared order and still produce a correct (if
suboptimal) layout.

### A.8 Layout Sketch

> Sizing constants: see [LAYOUT.md](LAYOUT.md) §4.2.4

With the sizing constants from section 4.2.4:

**Node dimensions** (using updated constants: HEADER_HEIGHT=32, ROW_HEIGHT=20,
CONTENT_PAD=12):

- ca: 3 properties → height = 32 + 3×20 = 92px. Width ≈ max("Certificate
  Authority"=21ch, "subject.common_name"=19ch) × 8 + 24 = 192px.
- cert: 6 properties → height = 32 + 6×20 = 152px. Width ≈
  max("Certificate"=11ch, "subject.common_name"=19ch) × 8 + 24 = 176px.
- tls: 2 properties → height = 32 + 2×20 = 72px. Width ≈ max("TLS Session"=11ch,
  "cipher_suite"=12ch) × 8 + 24 = 120px.
- revocation: 1 property → height = 32 + 1×20 = 52px. Width ≈ max("Revocation
  List"=15ch, "crl"=3ch) × 8 + 24 = 144px.

**Approximate layout (y from top, x from left):**

```
y=0:    [ca: 192×92]                     [revocation: 144×52]
        x=0                               x=232

y=120:  [cert: 176×152]
        x=16

y=300:  [tls: 120×72]
        x=44
```

(These are approximate. Brandes-Kopf centering and domain/corridor padding
will adjust coordinates.)

**Domain bounding boxes:**

- PKI: encloses ca and cert, plus corridors and title area.
- Transport: encloses tls, plus corridors and title area.
- revocation: top-level, no domain box.

---

## See Also

- [OVERVIEW.md](OVERVIEW.md) -- Pipeline architecture and validation rules
- [GRAPH_MODEL.md](GRAPH_MODEL.md) -- Graph data structures and state propagation algorithm
- [SYNTAX.md](SYNTAX.md) -- Input language grammar and AST definitions
- [LAYOUT.md](LAYOUT.md) -- Layer assignment, crossing minimization, and coordinate placement
- [RENDERING.md](RENDERING.md) -- SVG output and visual styling
