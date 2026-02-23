# obgraph: Design Documentation

This directory contains the design specification for the obgraph project,
split by concern for focused reading and efficient AI context loading.

## Documents

| Document | ~Tokens | Description |
|---|---|---|
| [OVERVIEW.md](OVERVIEW.md) | 3,800 | Project overview, architecture, pipeline, crate design, decisions table, references |
| [GRAPH_MODEL.md](GRAPH_MODEL.md) | 4,400 | Six primitives, state model, constraints/derivations, graph data structure, propagation algorithm |
| [SYNTAX.md](SYNTAX.md) | 2,800 | Input syntax grammar, PEG, complete example |
| [LAYOUT.md](LAYOUT.md) | 12,500 | Modified Sugiyama algorithm: layer assignment, crossing minimization, coordinate assignment, edge routing |
| [RENDERING.md](RENDERING.md) | 6,500 | SVG output, interactivity model, CSS/JS, color palette, visual design |
| [WORKED_EXAMPLE.md](WORKED_EXAMPLE.md) | 4,600 | End-to-end trace of the PKI example through every pipeline phase |

## Reading Order

- **New to the project**: Start with [OVERVIEW.md](OVERVIEW.md), then [GRAPH_MODEL.md](GRAPH_MODEL.md)
- **Working on `parse/`**: [SYNTAX.md](SYNTAX.md) + [GRAPH_MODEL.md](GRAPH_MODEL.md) (sections 2.1, 2.4)
- **Working on `model/`**: [GRAPH_MODEL.md](GRAPH_MODEL.md) + [OVERVIEW.md](OVERVIEW.md) (section 7.2)
- **Working on `layout/`**: [LAYOUT.md](LAYOUT.md)
- **Working on `render/`**: [RENDERING.md](RENDERING.md)
- **Verifying an implementation**: [WORKED_EXAMPLE.md](WORKED_EXAMPLE.md) alongside the relevant doc

## Related Files

- [`../../examples/design_mockup.html`](../../examples/design_mockup.html) — Pixel-perfect SVG mockup validating the visual design
- [`../../examples/sev_snp_tpm.md`](../../examples/sev_snp_tpm.md) — AMD SEV-SNP + TPM attestation chain example
