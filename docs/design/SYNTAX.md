# Input Syntax

This document covers the obgraph input syntax and grammar: how graph
definitions are written inside fenced code blocks.  It is extracted from
§3 of the master design document and preserves the original section
numbering for traceability.

The six primitives of the graph model — Node, Property, Anchor,
Constraint, Derivation, and Domain — are defined formally in
[GRAPH_MODEL.md](GRAPH_MODEL.md) §2.1.  The complete example in §3.3
below is traced through the full pipeline in
[WORKED_EXAMPLE.md](WORKED_EXAMPLE.md).

---

## 3. Input Syntax

obgraph blocks are embedded in markdown as fenced code blocks:

````
```obgraph
<graph definition>
```
````

### 3.1 Grammar

#### Nodes

```
node <ident> [<display_name>] [@annotation...] {
  <property> [@annotation...]
  ...
}
```

- `ident`: bare identifier used in constraint and anchor references.
- `display_name`: optional quoted string used as the rendered label. If omitted,
  the ident is rendered.
- Annotations: `@selected` (node's cross-domain constraints are visible by
  default), `@root` (anchor root — node is anchored by annotation; no parent
  anchor required). Others may be added in the future.

#### Properties

```
<property_name> [@annotation...]
```

- `property_name`: bare identifier or dotted path (e.g., `subject.common_name`).
  The engine does not interpret dots — they are part of the name.
- Annotations: `@critical` (participates in the node's `verified` computation —
  the node is not verified until this property is constrained), `@constrained`
  (pre-satisfied by annotation — no incoming constraint required). Properties
  with no annotations are informational: they do not affect node state.

#### Domains

```
domain <display_name> {
  node ... { ... }
  node ... { ... }
}
```

- `display_name`: quoted string rendered as the domain label.
- Domains are flat (no nesting). A node is either top-level or in exactly one
  domain.
- Domains have no identifier — they are purely visual.

#### Anchors

```
<child_ident> <- <parent_ident> [: <operation>]
```

- Hierarchical edge that anchors a child node to a parent. Anchoring flows
  right-to-left, matching constraint syntax. A valid anchor requires the parent
  to be both anchored and verified.
- Operation is optional. If present, it names the integrity method used to
  establish the anchor.

#### Constraints

```
<node>::<property> <= <source_expression> [: <operation>]
```

Trust flows right-to-left. The left-hand side is the property being verified.
The right-hand side is the source of trust. The `<=` operator reads as "is
constrained by."

The source expression is either a property reference or an inline derivation
(function call).

Simple constraint (default operation is equality):

```
cert::issuer.common_name <= ca::subject.common_name
```

Named constraint operation:

```
cert::signature <= ca::public_key : verified_by
```

Constraint with inline derivation as source:

```
cert::subject.common_name <= difference(all_names::list, revocation::crl) : not_in
```

Nested derivations:

```
cert::foo <= intersect(difference(X::bar, Y::crl), Z::approved) : subset_of
```

> Constraint vs derivation semantics: see [GRAPH_MODEL.md](GRAPH_MODEL.md) §2.4

#### Comments

```
# This is a comment
```

### 3.2 Formal Grammar (PEG)

```peg
# Top-level
graph       ← (comment / domain / node_decl / anchor / constraint / blank_line)*

# Lexical
ident       ← [a-zA-Z_] [a-zA-Z0-9_]*
prop_name   ← ident ('.' ident)*
string_lit  ← '"' [^"]* '"'
comment     ← '#' [^\n]* '\n'
trailing    ← _ ('#' [^\n]*)?               # optional trailing comment
blank_line  ← _ '\n'
_           ← [ \t]*                          # horizontal whitespace

# Nodes
node_decl   ← _ 'node' _ ident _ string_lit? _ (_ node_annot)* _ '{' _ trailing? '\n'
              prop_list
              _ '}' _ trailing? '\n'
node_annot  ← '@root' / '@selected'
prop_list   ← (_ prop_decl _ trailing? '\n' / blank_line)*
prop_decl   ← prop_name _ prop_annot*
prop_annot  ← '@critical' / '@constrained'

# Domains
domain      ← _ 'domain' _ string_lit _ '{' _ trailing? '\n'
              node_decl*
              _ '}' _ trailing? '\n'

# Anchors
anchor      ← _ ident _ '<-' _ ident _ (':' _ ident)? _ trailing? '\n'

# Constraints
constraint  ← _ prop_ref _ '<=' _ source_expr _ (':' _ ident)? _ trailing? '\n'
prop_ref    ← ident '::' prop_name
source_expr ← derivation / prop_ref
derivation  ← ident '(' _ arg_list _ ')'
arg_list    ← source_expr (_ ',' _ source_expr)*
```

**Lexical rules:**

- Identifiers (`ident`): ASCII letters, digits, and underscores. Must start with
  a letter or underscore.
- Property names (`prop_name`): one or more identifiers joined by dots. The
  engine does not interpret the dots — `subject.common_name` is an opaque name.
- Operation names (in anchors and constraints): follow `ident` rules.
- Derivation function names: follow `ident` rules.
- String literals: double-quoted, no escape sequences. Used only for display
  names.
- Whitespace: spaces and tabs are insignificant except as separators. Newlines
  are significant as statement terminators.
- Comments: `#` to end of line. May appear on their own line or after a
  statement.

### 3.3 Complete Example

> This example is traced step-by-step in [WORKED_EXAMPLE.md](WORKED_EXAMPLE.md).

```obgraph
# Domains are visual grouping only
domain "PKI" {
  node ca "Certificate Authority" @root @selected {
    subject.common_name    @constrained  # self-attesting on a root node
    subject.org            @constrained
    public_key             @constrained
  }

  node cert "Certificate" {
    issuer.common_name     @critical
    issuer.org             @critical
    subject.common_name                  # informational (not critical)
    subject.org                          # informational
    public_key             @constrained  # self-generated keypair
    signature              @critical
  }
}

domain "Transport" {
  node tls "TLS Session" {
    server_cert            @critical
    cipher_suite                         # informational
  }
}

node revocation "Revocation List" @root {
  crl                      @constrained  # self-attesting on a root node
}

# Anchors (anchoring flows right to left)
cert <- ca : sign
tls <- cert

# Constraints (trust flows right to left)
cert::issuer.common_name <= ca::subject.common_name
cert::issuer.org <= ca::subject.org
cert::signature <= ca::public_key : verified_by
tls::server_cert <= cert::public_key
cert::subject.common_name <= revocation::crl : not_in
```

### 3.4 Reference: Node-Property Separator

The `::` separator is used between node identifiers and property names (e.g.,
`ca::public_key`). This frees the dot character for use within property names
(e.g., `subject.common_name`) without requiring quoting.

---

## See Also

- [GRAPH_MODEL.md](GRAPH_MODEL.md) — graph primitives, state semantics, and constraint/derivation rules
- [WORKED_EXAMPLE.md](WORKED_EXAMPLE.md) — the §3.3 example traced through the full pipeline
- [LAYOUT.md](LAYOUT.md) — layout algorithm (Sugiyama adaptation)
- [RENDERING.md](RENDERING.md) — SVG rendering and interactivity
