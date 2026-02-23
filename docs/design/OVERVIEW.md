# obgraph: Overview, Architecture, and Design Decisions

> **Scope.** This document combines the high-level overview (section 1), architecture
> and crate design (section 7), summary of design decisions (section 8), and
> references (section 9) from the obgraph design document. Section numbering is
> preserved for traceability back to the original DESIGN.md.

---

## 1. Overview

obgraph is a directed acyclic graph format and rendering engine designed to
visualize state propagation through systems of interconnected objects. It is
implemented as `mdbook-obgraph`, an mdbook preprocessor that parses `obgraph`
code blocks in markdown and emits inline SVG with minimal JavaScript for
interactivity.

The name "observational graph" reflects the core semantic: an observer observes
some process and records a subset of properties about that process. A child node
is that recorded subset. A Certificate node is not a self-describing object — it
is the subset of properties that the Certificate Authority observed and recorded
about the signing process. The graph models these recorded observations and the
constraints that must hold between them for trust to propagate through the
system.

The primary use case is modeling attestation chains, certificate hierarchies,
and similar systems where properties of objects must be verified through
directed constraints, and trust flows from axiomatic sources through those
constraints to establish the integrity of the system.

## 7. Architecture

### 7.0 Implementation Guidance

The pseudocode in this document is written for algorithmic clarity, not as a
style guide. All output code must be **idiomatic Rust**:

- **Object-oriented style**: Prefer methods on structs/enums over free
  functions. For example, `graph.layout_endpoints(edge)` not
  `layout_endpoints(graph, edge)`. Algorithms should be implemented as methods
  on the types they operate on.
- **No unsafe code**: The crate must include `#![forbid(unsafe_code)]` at the
  crate root. There is no performance requirement that justifies unsafe in this
  project.
- **Standard Rust conventions**: Use `Result<T, E>` for fallible operations,
  `impl` blocks for associated functions, `Iterator` trait implementations where
  appropriate, and derive macros (`Debug`, `Clone`, etc.) on all public types.

### 7.1 Pipeline

````
Markdown with ```obgraph code blocks
  → mdbook-obgraph preprocessor (Rust)
    → Parse: text → AST                                   → see [SYNTAX.md](SYNTAX.md)
    → Validate: enforce rules from section 7.2
    → Deduplicate: normalize and compare derivation expressions, merge duplicates
    → State: propagate anchored/verified flags (section 2.6)  → see [GRAPH_MODEL.md](GRAPH_MODEL.md)
    → Layout: modified Sugiyama pipeline (phases 2–6)      → see [LAYOUT.md](LAYOUT.md)
    → Render: emit SVG + inline CSS + inline JS            → see [RENDERING.md](RENDERING.md)
  → mdbook renders HTML with embedded graphs
````

### 7.2 Validation Rules

The parser and validator reject the following:

| Rule                                    | Error                                                                                        |
| --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Duplicate node identifier               | Two nodes with the same ident                                                                |
| Duplicate property name within a node   | Two properties in the same node with the same name                                           |
| Reference to nonexistent node           | Constraint or anchor references a node ident that doesn't exist                              |
| Reference to nonexistent property       | Constraint references a property name that doesn't exist on the specified node               |
| Constraint on `@constrained` property   | A `@constrained` property has an incoming constraint (redundant — already pre-satisfied)     |
| `@root` node with incoming anchor       | A node annotated `@root` is the child (left-hand side) of an anchor                          |
| Multiple incoming anchors               | A node appears as the child in more than one anchor                                          |
| Nullary derivation                      | A derivation function call with zero arguments                                               |
| Cycle detected                          | The combined graph of anchors, derivation edges, and constraints contains a cycle            |

State computation rules: see [GRAPH_MODEL.md](GRAPH_MODEL.md).

### 7.3 Crate Design

The project follows the standard mdbook preprocessor pattern: a single crate
with a library and a binary. The binary handles the mdbook preprocessor protocol
(stdin/stdout JSON). The library contains all parsing, layout, and rendering
logic and is independently usable outside of mdbook.

#### Source Tree

```
mdbook-obgraph/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Public API: parse → validate → layout → render
│   ├── bin/
│   │   └── mdbook_obgraph.rs     # CLI: preprocessor protocol + install subcommand
│   ├── parse/
│   │   ├── mod.rs                # parse(&str) → Result<Ast>
│   │   ├── lexer.rs              # Tokenizer
│   │   ├── ast.rs                # Unvalidated AST types
│   │   └── dedup.rs              # Derivation expression normalization and deduplication
│   ├── model/
│   │   ├── mod.rs                # Validated graph construction from AST
│   │   ├── types.rs              # Node, Property, Anchor, Constraint, Derivation, Domain
│   │   ├── graph.rs              # Purpose-built immutable graph with port-level adjacency
│   │   ├── state.rs              # Fixed-point state propagation (anchored/constrained)
│   │   └── validate.rs           # Validation rules (section 7.2)
│   ├── layout/
│   │   ├── mod.rs                # layout(&Graph) → LayoutResult
│   │   ├── layer_assign.rs       # Network simplex with typed layer pre-assignment
│   │   ├── long_edge.rs          # Eiglsperger implicit long edge segments
│   │   ├── crossing.rs           # Weighted port-aware nested barycenter
│   │   ├── coordinate.rs         # Brandes-Köpf coordinate assignment
│   │   ├── routing.rs            # Orthogonal channel-based edge routing
│   │   └── domain.rs             # Domain bounding box computation
│   └── render/
│       ├── mod.rs                # render(&LayoutResult) → String (HTML fragment)
│       ├── svg.rs                # SVG element generation
│       ├── style.rs              # Inline CSS (edge styles, state colors)
│       └── interactivity.rs      # Inline JS (hover/click visibility toggling)
```

Module documentation references:

- `parse/` -- see [SYNTAX.md](SYNTAX.md)
- `model/` -- see [GRAPH_MODEL.md](GRAPH_MODEL.md)
- `layout/` -- see [LAYOUT.md](LAYOUT.md)
- `render/` -- see [RENDERING.md](RENDERING.md)
- `render/interactivity.rs` -- see [RENDERING.md](RENDERING.md) §5

#### Module Boundaries

Each module has a clean input/output contract. This enables independent unit
testing of each phase.

```
parse::parse(&str)
  → Result<Ast>

model::build(Ast)
  → Result<Graph>            # validates, deduplicates, builds graph

model::state::propagate(&Graph)
  → StateResult              # per-node, per-property state flags

layout::layout(&Graph)
  → LayoutResult              # coordinates, edge paths, stub paths

render::render(&LayoutResult, &StateResult)
  → String                    # self-contained HTML/SVG fragment
```

The top-level library API composes these:

```rust
pub fn process(input: &str) -> Result<String> {
    let ast = parse::parse(input)?;
    let graph = model::build(ast)?;
    let state = model::state::propagate(&graph);
    let layout = layout::layout(&graph)?;
    Ok(render::render(&layout, &state))
}
```

#### Binary: Preprocessor Protocol

The binary implements the mdbook preprocessor protocol:

- `mdbook-obgraph supports <renderer>`: returns `0` for `html`, `1` otherwise.
- `mdbook-obgraph`: reads `[context, book]` JSON from stdin, walks all chapters,
  replaces ` ```obgraph ` code blocks with the output of `process()`, writes
  modified book JSON to stdout.
- `mdbook-obgraph install <path>`: adds `[preprocessor.obgraph]` to `book.toml`.
  Unlike mdbook-mermaid, no external JS/CSS files are needed — all output is
  self-contained inline SVG.

#### Configuration

Optional settings in `book.toml`:

```toml
[preprocessor.obgraph]
command = "mdbook-obgraph"
```

Future options (not implemented in v1):

```toml
[preprocessor.obgraph]
# Color theme (default: "light")
theme = "dark"
# Error handling: "fail" (default) or "inline" (render error message in place)
on-error = "fail"
```

#### Dependencies

- `mdbook`: preprocessor interface (binary only; the library has no mdbook
  dependency).
- `clap`: CLI argument parsing.
- `serde`, `serde_json`: mdbook preprocessor protocol.
- Standard Rust crates for SVG generation (or direct string templating).

Note: `petgraph` is **not** used. See section 7.4 for rationale.

### 7.4 Implementation Approach: Custom Layout Engine

#### Why Not Existing Libraries

The following Rust graph layout libraries were evaluated and rejected:

| Library                    | What It Provides                                                                                         | What It Lacks                                                                                                                                             |
| -------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust-sugiyama` (paddison) | Network simplex layer assignment, barycenter/weighted median crossing minimization, petgraph integration | No port awareness, no edge weights, no edge routing, no variable node sizes. Point-coordinate output only.                                                |
| `dagre-rs` (TangleGuard)   | Port of dagre.js to Rust                                                                                 | Very immature (11 commits, 7 stars). Dagre.js never supported ports (open issue #106 from 2013). Dagre.js unmaintained since 2018. No orthogonal routing. |
| `layout-rs` (nicebyte)     | GraphViz DOT parser, record-style nodes, SVG output                                                      | Opinionated end-to-end pipeline. Cannot inject typed layers, custom crossing minimization, or port-aware optimization.                                    |

The only layout engine that supports all of obgraph's requirements (port-aware
nested barycenter, typed layers, weighted multi-edge-type crossing minimization,
orthogonal channel routing) is **ELK** (Eclipse Layout Kernel), which is written
in Java and available as `elkjs` via GWT transpilation to JavaScript. There is
no native Rust port. Using ELK would require either shelling out to a JS runtime
or embedding a WASM blob — neither is appropriate for an mdbook preprocessor
that should be a single static binary.

The existing Rust libraries cover at most 30% of the required functionality.
Extending any of them would mean fighting their internal abstractions (which
assume point nodes with unweighted edges) rather than leveraging them.

#### Custom Implementation

The layout engine is implemented from scratch in Rust. Each Sugiyama phase is a
standalone module operating on a shared graph representation:

1. **Layer assignment**: network simplex with typed layer pre-assignment
   (section 4.2.2)
2. **Long edge handling**: Eiglsperger implicit segments (section 4.2.2)
3. **Crossing minimization**: weighted port-aware nested barycenter (section
   4.2.3)
4. **Coordinate assignment**: Brandes-Köpf (section 4.2.4)
5. **Edge routing**: orthogonal channel-based routing (section 4.2.6)

#### Why Not petgraph

`petgraph` was evaluated for the model graph and rejected. The obgraph model is
port-centric: edges connect properties (ports) on nodes, not nodes to nodes.
petgraph has no concept of sub-node endpoints. Using it would require:

- Enum dispatch (`NodeWeight::Regular` / `NodeWeight::Derivation`) on every node
  access.
- Enum dispatch (`EdgeWeight::Anchor` / `EdgeWeight::Constraint` /
  `EdgeWeight::DerivInput` / ...) on every edge access.
- A parallel port index (`HashMap<(NodeId, PropIdx), Vec<EdgeId>>`) alongside
  the petgraph structure, since crossing minimization needs per-property
  adjacency queries.

At that point, the layout code primarily queries the port index and barely
touches petgraph. Two parallel representations of the same graph that must stay
in sync is worse than one purpose-built structure.

The algorithms petgraph would provide — topological sort and cycle detection —
are trivial for DAGs (Kahn's algorithm is ~20 lines; cycle detection is "did
topo sort visit all nodes?"). The graph model is small, immutable after
construction, and has a fixed set of six primitives. A purpose-built immutable
graph with first-class port-level adjacency is simpler and more maintainable
than a general-purpose mutable graph with a sidecar index.

## 8. Summary of Design Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| Implementation | Custom layout engine and custom graph | No Rust layout library supports our requirements. `petgraph` rejected: port-centric model requires parallel sidecar index, making petgraph overhead without benefit. |
| Layout algorithm | Modified Sugiyama | Best framework for hierarchical DAGs. Well-studied, efficient, extensible. |
| Layer assignment | Network simplex (Gansner et al., 1993) on all edge types | Gold standard for minimizing total edge length. Handles weighted edges naturally. |
| Typed layers | Pre-assignment via layer parity (nodes=even, derivations=odd) | Encodes type constraint directly in network simplex, avoiding post-processing that could invalidate edge directions. |
| Long edge handling | Eiglsperger implicit segments | Keeps algorithm complexity proportional to real graph size rather than rendered edge lengths. Critical for constraint edges spanning many layers. |
| Derivation rendering | Rounded pills (not diamonds); always visible as landmarks | Simpler to implement, more legible at small sizes. Pills serve as landmarks indicating derivation existence even when edges are hidden. |
| Derivation edge colors | Same blue/red as constraints (not gray) | Consistent valid/invalid visual language across all trust-propagation edges. |
| State visualization | Problems-only (red dots for problems; no indicator for OK) | Less visual noise. Draws attention only where action is needed. Verified/unverified is emergent from property indicators. |
| Node header | Always neutral color regardless of verified state | Problems-only principle: headers never change color. Verified state is emergent. |
| Selection ring | 2px dark stroke ring, fully inset | Orthogonal to anchored/verified state. Ring inset 1px so outer edge = rect boundary. Edges land cleanly, no overlap. |
| Sizing grid | 4px base unit; all dimensions even | Produces a compact, polished result. Coordinates snap cleanly to grid. |
| Crossing minimization | Weighted barycenter with nested port optimization | Handles three edge types with priority. Port-aware for record-style nodes. Based on ELK approach. |
| Edge crossing weights | Anchors=3, derivations=2, constraints=1 | Anchors are the primary structural edges. Derivation edges are secondary structure. Constraints are tertiary. |
| Port rendering | Invisible attachment points (no port circles) | Cleaner visual. Problem dots (2px radius) serve as the only indicators. |
| Edge routing | Orthogonal, corridor-based, all pre-computed | Maximum visual order and symmetry. Corridors provide deterministic channel allocation. |
| Constraint visibility | Intra-domain: always visible. Cross-domain: stub by default, full on hover/select. | Balances information density with readability. Stub arrows indicate hidden edges exist. |
| Derivation chain visibility | Atomic: selecting any participant reveals entire chain; pill always visible | Derivation chains are logical units. Partial reveal would be confusing. Pill as landmark aids comprehension. |
| Arrowhead design | All 6×6; refX=0; `markerUnits="userSpaceOnUse"` | Uniform size and clearance. Path stops 6px short; arrowhead fills gap. Prevents stroke bleed. Edge types distinguished by color/stroke width. |
| Anchor edge labels | Operation name rendered as label on edge | Useful visual information showing the integrity method. |
| Anchor direction | Right-to-left in all statements | Anchors and constraints both flow trust from right to left. |
| Node anchor annotation | `@root` | Single keyword, distinct from property annotations. Indicates anchor root. |
| Property state annotations | `@critical` / `@constrained` | Two independent binary flags. `@critical` gates node verified state; `@constrained` pre-satisfies the constraint slot. |
| Derivation deduplication | String equality of normalized expressions | Eliminates redundancy without requiring author-side naming. |
| Syntax: node-property separator | `::` | Frees dot for use in property names without quoting. |
| Syntax: anchor direction | `cert <- ca : sign` | Right-to-left, matching constraint direction. Consistent trust flow. |
| Syntax: constraint operator | `<=` | Reads as "is constrained by." Visually distinct from `<-` (anchors). |
| Syntax: constraint operation | `cert::sig <= ca::key : verified_by` | Constraint operation is a colon-suffixed name, same pattern as anchors. Optional; default is equality. |
| Syntax: derivations | `difference(X::bar, Y::crl)` | Function-call syntax for value-producing computations. Inline only; parser deduplicates identical expressions. |
| JavaScript footprint | Minimal: CSS visibility toggling only | All geometry pre-computed. No dynamic layout or edge routing in the browser. |
| Render format | Inline SVG + CSS + JS in HTML | Self-contained, no external dependencies. Compatible with mdbook output. |

## 9. References

- Sugiyama, K., Tagawa, S., & Toda, M. (1981). Methods for visual understanding
  of hierarchical system structures. _IEEE Transactions on Systems, Man and
  Cybernetics_, 11(2), 109-125.
- Gansner, E., Koutsofios, E., North, S., & Vo, K. (1993). A technique for
  drawing directed graphs. _IEEE Transactions on Software Engineering_, 19(3),
  214-229.
- Eiglsperger, M., Siebenhaller, M., & Kaufmann, M. (2005). An efficient
  implementation of Sugiyama's algorithm for layered graph drawing. _Graph
  Drawing (GD 2004)_, LNCS 3383, 155-166.
- Brandes, U. & Kopf, B. (2001). Fast and simple horizontal coordinate
  assignment. _Graph Drawing (GD 2001)_, LNCS 2265, 31-44.
- Dobler, A. et al. (2025). Layered graph drawing with few gaps and few
  crossings. arXiv:2502.20896.
- Domros, S. & von Hanxleden, R. (2024). Determining Sugiyama topology with
  model order. _32nd International Symposium on Graph Drawing and Network
  Visualization (GD 2024)_, LIPIcs 320.
- Caroppo, S., Da Lozzo, G., & Di Battista, G. (2024). Quantum algorithms for
  one-sided crossing minimization. _GD 2024_, LIPIcs 320.
- Healy, P. & Nikolov, N. (2013). Hierarchical drawing algorithms. In Tamassia,
  R. (ed.), _Handbook of Graph Drawing and Visualization_, CRC Press,
  Chapter 13.

---

## See Also

- [SYNTAX.md](SYNTAX.md) -- Input syntax (section 3)
- [GRAPH_MODEL.md](GRAPH_MODEL.md) -- Graph model, state propagation, and validation (section 2)
- [LAYOUT.md](LAYOUT.md) -- Layout algorithm: modified Sugiyama pipeline (section 4)
- [RENDERING.md](RENDERING.md) -- Rendering, interactivity, and visual specification (sections 5-6)
- [WORKED_EXAMPLE.md](WORKED_EXAMPLE.md) -- Worked example tracing through all pipeline phases (Appendix A)
