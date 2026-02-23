/// Inline CSS for obgraph SVG elements (DESIGN.md §6.8).

/// Returns the complete inline CSS for an obgraph SVG.
pub fn css() -> &'static str {
    r#"
.obgraph {
  background: #ffffff;
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  font-size: 9px;
  color: #1e293b;
  overflow: visible;
}

/* Domain background and label */
.obgraph-domain-bg {
  fill: #f8fafc;
  stroke: #94a3b8;
  stroke-width: 1.5px;
}

.obgraph-domain-label {
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  font-size: 9px;
  font-weight: 600;
  fill: #475569;
}

/* Link edges — green */
.obgraph-link {
  fill: none;
  stroke: #22c55e;
  stroke-width: 1.5px;
}

/* Derivation edges */
.obgraph-deriv-edge {
  fill: none;
  stroke: #94a3b8;
  stroke-width: 1px;
}

/* Intra-domain constraint — blue */
.obgraph-constraint {
  fill: none;
  stroke: #3b82f6;
  stroke-width: 1.2px;
}

/* Cross-domain constraint full path — amber, hidden by default */
.obgraph-constraint-full {
  fill: none;
  stroke: #f59e0b;
  stroke-width: 1.5px;
  stroke-dasharray: 4 3;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}

.obgraph-constraint-full.obgraph-active {
  opacity: 1;
  pointer-events: auto;
}

/* Cross-domain constraint stub — amber */
.obgraph-constraint-stub {
  fill: none;
  stroke: #f59e0b;
  stroke-width: 1.2px;
  stroke-dasharray: 4 3;
  transition: opacity 0.15s ease;
}

.obgraph-constraint-stub.obgraph-hidden {
  opacity: 0;
  pointer-events: none;
}

/* Link-source property highlight */
.obgraph-link-source {
  fill: #dcfce7;
}

/* Node background rect — white with dark slate border */
.obgraph-node-bg {
  fill: #ffffff;
  stroke: #334155;
  stroke-width: 1.5px;
}

/* Node title text — dark, no header background */
.obgraph-node-title {
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  font-size: 9px;
  font-weight: 600;
  fill: #1e293b;
}

/* Trusted node title — teal tint */
.obgraph-node-title[data-trust="trusted"] {
  fill: #0f766e;
}

/* Separator line between title and properties */
.obgraph-node-sep {
  stroke: #e2e8f0;
  stroke-width: 1px;
}

/* Property row background */
.obgraph-prop-bg {
  fill: transparent;
}

/* Property name text */
.obgraph-prop-name {
  fill: #64748b;
  font-size: 8px;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
}

/* Trusted property name */
.obgraph-prop[data-trust="trusted"] .obgraph-prop-name {
  fill: #475569;
}

/* Always-trusted property name */
.obgraph-prop[data-trust="always"] .obgraph-prop-name {
  fill: #0f766e;
  font-weight: 600;
}

/* Derivation diamond shape */
.obgraph-deriv-shape {
  fill: #f8fafc;
  stroke: #94a3b8;
  stroke-width: 1px;
}

/* Derivation label */
.obgraph-deriv-label {
  fill: #64748b;
  font-size: 8px;
  text-anchor: middle;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
}

/* Arrowhead markers */
.obgraph-arrow-link {
  fill: #22c55e;
}

.obgraph-arrow-constraint {
  fill: #3b82f6;
}

.obgraph-arrow-constraint-cross {
  fill: #f59e0b;
}

/* Edge operation labels */
.obgraph-link-label {
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  font-size: 8px;
  fill: #16a34a;
  paint-order: stroke;
  stroke: white;
  stroke-width: 3px;
  stroke-linejoin: round;
}

.obgraph-constraint-label {
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  font-size: 8px;
  fill: #2563eb;
  paint-order: stroke;
  stroke: white;
  stroke-width: 3px;
  stroke-linejoin: round;
}

/* Selected node highlight */
.obgraph-node[data-selected="true"] .obgraph-node-bg {
  stroke: #3b82f6;
  stroke-width: 2px;
}
"#
}
