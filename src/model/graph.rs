/// Graph construction from AST (DESIGN.md §2.5, Appendix A.3).

use std::collections::HashMap;

use crate::parse::ast::{AstDerivationExpr, AstGraph, AstSourceExpr, AstTrustAnnotation};
use crate::parse::dedup::DerivDedup;
use crate::ObgraphError;

use super::types::{
    DerivId, Derivation, Domain, DomainId, Edge, EdgeId, Graph, Node, NodeId, PropId, Property,
    TrustClass,
};
use super::validate;

// ---------------------------------------------------------------------------
// Internal builder state
// ---------------------------------------------------------------------------

struct Builder {
    nodes: Vec<Node>,
    properties: Vec<Property>,
    derivations: Vec<Derivation>,
    edges: Vec<Edge>,
    domains: Vec<Domain>,

    prop_edges: HashMap<PropId, Vec<EdgeId>>,
    node_children: HashMap<NodeId, Vec<EdgeId>>,
    node_parent: HashMap<NodeId, EdgeId>,

    /// Map from (node_ident, prop_name) -> PropId for quick lookup.
    prop_lookup: HashMap<(String, String), PropId>,

    /// Map from node_ident -> NodeId for quick lookup.
    node_lookup: HashMap<String, NodeId>,

    /// Map from normalized derivation string -> DerivId for deduplication.
    /// The corresponding output PropId is stored in the Derivation itself.
    deriv_lookup: HashMap<String, DerivId>,

    dedup: DerivDedup,
}

impl Builder {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            properties: Vec::new(),
            derivations: Vec::new(),
            edges: Vec::new(),
            domains: Vec::new(),
            prop_edges: HashMap::new(),
            node_children: HashMap::new(),
            node_parent: HashMap::new(),
            prop_lookup: HashMap::new(),
            node_lookup: HashMap::new(),
            deriv_lookup: HashMap::new(),
            dedup: DerivDedup::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Allocation helpers
    // -----------------------------------------------------------------------

    fn next_node_id(&self) -> NodeId {
        NodeId(self.nodes.len() as u32)
    }

    fn next_prop_id(&self) -> PropId {
        PropId(self.properties.len() as u32)
    }

    fn next_deriv_id(&self) -> DerivId {
        DerivId(self.derivations.len() as u32)
    }

    fn next_edge_id(&self) -> EdgeId {
        EdgeId(self.edges.len() as u32)
    }

    fn next_domain_id(&self) -> DomainId {
        DomainId(self.domains.len() as u32)
    }

    // -----------------------------------------------------------------------
    // Node allocation
    // -----------------------------------------------------------------------

    fn alloc_node(
        &mut self,
        ident: &str,
        display_name: Option<String>,
        is_root: bool,
        is_selected: bool,
        domain: Option<DomainId>,
    ) -> Result<NodeId, ObgraphError> {
        if self.node_lookup.contains_key(ident) {
            return Err(ObgraphError::Validation(format!(
                "duplicate node identifier: {ident}"
            )));
        }
        let id = self.next_node_id();
        let node = Node {
            id,
            ident: ident.to_string(),
            display_name,
            properties: Vec::new(),
            domain,
            is_root,
            is_selected,
        };
        self.nodes.push(node);
        self.node_lookup.insert(ident.to_string(), id);
        Ok(id)
    }

    // -----------------------------------------------------------------------
    // Property allocation
    // -----------------------------------------------------------------------

    fn alloc_property(
        &mut self,
        node_id: NodeId,
        node_ident: &str,
        name: &str,
        trust: TrustClass,
    ) -> Result<PropId, ObgraphError> {
        let key = (node_ident.to_string(), name.to_string());
        if self.prop_lookup.contains_key(&key) {
            return Err(ObgraphError::Validation(format!(
                "duplicate property {name} on node {node_ident}"
            )));
        }
        let id = self.next_prop_id();
        let prop = Property {
            id,
            node: node_id,
            name: name.to_string(),
            trust,
        };
        self.properties.push(prop);
        self.prop_lookup.insert(key, id);
        // Register on the node.
        self.nodes[node_id.index()].properties.push(id);
        Ok(id)
    }

    /// Allocate an ephemeral output property for a derivation.
    /// The property is given a synthetic name and Always trust so it never
    /// gates node trust on its own.
    fn alloc_deriv_output_prop(&mut self, node_id: NodeId, synth_name: &str) -> PropId {
        let id = self.next_prop_id();
        let prop = Property {
            id,
            node: node_id,
            name: synth_name.to_string(),
            trust: TrustClass::Always,
        };
        self.properties.push(prop);
        // Ephemeral props are NOT registered in prop_lookup (they are
        // not addressable by the user) and NOT added to the node's property
        // list (they exist only in the edge graph).
        id
    }

    // -----------------------------------------------------------------------
    // Edge helpers
    // -----------------------------------------------------------------------

    fn push_edge(&mut self, edge: Edge) -> EdgeId {
        let id = self.next_edge_id();
        self.edges.push(edge);
        id
    }

    fn record_prop_edge(&mut self, prop: PropId, edge: EdgeId) {
        self.prop_edges.entry(prop).or_default().push(edge);
    }

    // -----------------------------------------------------------------------
    // Resolve helpers
    // -----------------------------------------------------------------------

    fn resolve_node(&self, ident: &str) -> Result<NodeId, ObgraphError> {
        self.node_lookup.get(ident).copied().ok_or_else(|| {
            ObgraphError::Validation(format!("unknown node identifier: {ident}"))
        })
    }

    fn resolve_prop(&self, node_ident: &str, prop_name: &str) -> Result<PropId, ObgraphError> {
        let key = (node_ident.to_string(), prop_name.to_string());
        self.prop_lookup.get(&key).copied().ok_or_else(|| {
            ObgraphError::Validation(format!(
                "unknown property {prop_name} on node {node_ident}"
            ))
        })
    }

    // -----------------------------------------------------------------------
    // Derivation processing
    // -----------------------------------------------------------------------

    /// Recursively ensure a derivation expression is allocated in the graph,
    /// deduplicating by normalized string. Returns the output PropId of the
    /// (possibly pre-existing) derivation node.
    ///
    /// `dest_node_id` is used only when we need to synthesize an ephemeral
    /// output property — we attribute it to the destination node for
    /// layout purposes.
    fn ensure_derivation(
        &mut self,
        expr: &AstDerivationExpr,
        dest_node_id: NodeId,
    ) -> Result<PropId, ObgraphError> {
        let key = expr.normalized();

        // Fast path: already seen this derivation.
        if let Some(&deriv_id) = self.deriv_lookup.get(&key) {
            let output_prop = self.derivations[deriv_id.index()].output_prop;
            return Ok(output_prop);
        }

        // Slow path: create a new derivation.
        // 1. Resolve all input arguments to PropIds.
        let mut input_prop_ids: Vec<PropId> = Vec::new();
        for arg in &expr.args {
            let input_pid = self.resolve_source_to_prop(arg, dest_node_id)?;
            input_prop_ids.push(input_pid);
        }

        // 2. Allocate an ephemeral output property for this derivation.
        let deriv_id = self.next_deriv_id();
        let synth_name = format!("__deriv_{}", deriv_id.0);
        let output_prop = self.alloc_deriv_output_prop(dest_node_id, &synth_name);

        // 3. Allocate the derivation.
        let deriv = Derivation {
            id: deriv_id,
            operation: expr.function.clone(),
            inputs: input_prop_ids.clone(),
            output_prop,
        };
        self.derivations.push(deriv);
        self.deriv_lookup.insert(key, deriv_id);

        // 4. Create DerivInput edges for each input.
        for src_pid in input_prop_ids {
            let eid = self.push_edge(Edge::DerivInput {
                source_prop: src_pid,
                target_deriv: deriv_id,
            });
            self.record_prop_edge(src_pid, eid);
        }

        Ok(output_prop)
    }

    /// Resolve any AstSourceExpr to a PropId (possibly creating derivations).
    fn resolve_source_to_prop(
        &mut self,
        source: &AstSourceExpr,
        dest_node_id: NodeId,
    ) -> Result<PropId, ObgraphError> {
        match source {
            AstSourceExpr::PropRef {
                node_ident,
                prop_name,
            } => self.resolve_prop(node_ident, prop_name),
            AstSourceExpr::Derivation(deriv_expr) => {
                self.ensure_derivation(deriv_expr, dest_node_id)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Final graph assembly
    // -----------------------------------------------------------------------

    fn finish(self) -> Graph {
        Graph {
            nodes: self.nodes,
            properties: self.properties,
            derivations: self.derivations,
            edges: self.edges,
            domains: self.domains,
            prop_edges: self.prop_edges,
            node_children: self.node_children,
            node_parent: self.node_parent,
        }
    }
}

// ---------------------------------------------------------------------------
// TrustClass conversion
// ---------------------------------------------------------------------------

fn trust_class_from(ann: AstTrustAnnotation) -> TrustClass {
    match ann {
        AstTrustAnnotation::Default => TrustClass::Critical,
        AstTrustAnnotation::Constrained => TrustClass::Constrained,
        AstTrustAnnotation::Always => TrustClass::Always,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a validated `Graph` from a parsed AST.
///
/// This function:
/// 1. Allocates all nodes, properties, derivations, domains.
/// 2. Resolves all references (node idents, property names).
/// 3. Deduplicates derivation expressions.
/// 4. Builds the adjacency indices (prop_edges, node_children, node_parent).
/// 5. Runs validation.
///
/// Returns the immutable `Graph` or an error.
pub fn build(ast: AstGraph) -> Result<Graph, ObgraphError> {
    let mut b = Builder::new();

    // ------------------------------------------------------------------
    // Phase 1: Allocate domains and their member nodes (in order).
    // ------------------------------------------------------------------
    for ast_domain in &ast.domains {
        let domain_id = b.next_domain_id();
        let mut member_ids: Vec<NodeId> = Vec::new();

        for ast_node in &ast_domain.nodes {
            let node_id = b.alloc_node(
                &ast_node.ident,
                ast_node.display_name.clone(),
                ast_node.is_root,
                ast_node.is_selected,
                Some(domain_id),
            )?;
            member_ids.push(node_id);

            for ast_prop in &ast_node.properties {
                let trust = trust_class_from(ast_prop.trust);
                b.alloc_property(node_id, &ast_node.ident, &ast_prop.name, trust)?;
            }
        }

        let domain = Domain {
            id: domain_id,
            display_name: ast_domain.display_name.clone(),
            members: member_ids,
        };
        b.domains.push(domain);
    }

    // ------------------------------------------------------------------
    // Phase 2: Allocate top-level nodes (no domain).
    // ------------------------------------------------------------------
    for ast_node in &ast.nodes {
        let node_id = b.alloc_node(
            &ast_node.ident,
            ast_node.display_name.clone(),
            ast_node.is_root,
            ast_node.is_selected,
            None,
        )?;

        for ast_prop in &ast_node.properties {
            let trust = trust_class_from(ast_prop.trust);
            b.alloc_property(node_id, &ast_node.ident, &ast_prop.name, trust)?;
        }
    }

    // ------------------------------------------------------------------
    // Phase 3: Process links.
    // ------------------------------------------------------------------
    for ast_link in &ast.links {
        let child_id = b.resolve_node(&ast_link.child_ident)?;
        let parent_id = b.resolve_node(&ast_link.parent_ident)?;

        let eid = b.push_edge(Edge::Link {
            child: child_id,
            parent: parent_id,
            operation: ast_link.operation.clone(),
        });

        // node_children: parent -> [edge_ids for each child link]
        b.node_children.entry(parent_id).or_default().push(eid);

        // node_parent: child -> edge_id of its parent link
        // If a child already has a parent, that's a validation error.
        if b.node_parent.contains_key(&child_id) {
            return Err(ObgraphError::Validation(format!(
                "node {} has more than one parent link",
                ast_link.child_ident
            )));
        }
        b.node_parent.insert(child_id, eid);
    }

    // ------------------------------------------------------------------
    // Phase 4: Process constraints.
    // ------------------------------------------------------------------
    // We collect constraints first so we can borrow ast without mutable
    // aliasing issues while mutating `b`.
    let constraints: Vec<_> = ast.constraints.iter().collect();

    for ast_constraint in constraints {
        let dest_node_id = b.resolve_node(&ast_constraint.dest_node)?;
        let dest_prop_id = b.resolve_prop(&ast_constraint.dest_node, &ast_constraint.dest_prop)?;

        // Dedup the source expression (for derivations).
        let deduped_source = b.dedup.dedup(ast_constraint.source.clone());

        let source_prop_id = b.resolve_source_to_prop(&deduped_source, dest_node_id)?;

        let eid = b.push_edge(Edge::Constraint {
            dest_prop: dest_prop_id,
            source_prop: source_prop_id,
            operation: ast_constraint.operation.clone(),
        });

        b.record_prop_edge(dest_prop_id, eid);
        b.record_prop_edge(source_prop_id, eid);
    }

    // ------------------------------------------------------------------
    // Phase 5: Validate and return.
    // ------------------------------------------------------------------
    let graph = b.finish();
    validate::validate(&graph)?;
    Ok(graph)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ast::{
        AstConstraint, AstDerivationExpr, AstDomain, AstGraph, AstLink, AstNode, AstProperty,
        AstSourceExpr, AstTrustAnnotation,
    };

    // Helper to build a minimal AstNode (non-root).
    fn ast_node(ident: &str, props: Vec<(&str, AstTrustAnnotation)>) -> AstNode {
        ast_node_root(ident, props, false)
    }

    // Helper to build a minimal AstNode with explicit is_root flag.
    fn ast_node_root(ident: &str, props: Vec<(&str, AstTrustAnnotation)>, is_root: bool) -> AstNode {
        AstNode {
            ident: ident.to_string(),
            display_name: None,
            is_root,
            is_selected: false,
            properties: props
                .into_iter()
                .map(|(name, trust)| AstProperty {
                    name: name.to_string(),
                    trust,
                })
                .collect(),
        }
    }

    fn prop_ref(node: &str, prop: &str) -> AstSourceExpr {
        AstSourceExpr::PropRef {
            node_ident: node.to_string(),
            prop_name: prop.to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Test: empty graph
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_graph() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![],
            links: vec![],
            constraints: vec![],
        };
        let g = build(ast).expect("empty graph should build");
        assert_eq!(g.nodes.len(), 0);
        assert_eq!(g.properties.len(), 0);
        assert_eq!(g.edges.len(), 0);
        assert_eq!(g.domains.len(), 0);
        assert_eq!(g.derivations.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Test: single node with properties
    // -----------------------------------------------------------------------

    #[test]
    fn test_single_node_properties() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![ast_node_root(
                "ca",
                vec![
                    ("subject.common_name", AstTrustAnnotation::Always),
                    ("subject.org", AstTrustAnnotation::Always),
                    ("public_key", AstTrustAnnotation::Always),
                ],
                true,
            )],
            links: vec![],
            constraints: vec![],
        };
        let g = build(ast).expect("single node should build");

        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.properties.len(), 3);

        let node = &g.nodes[0];
        assert_eq!(node.ident, "ca");
        assert_eq!(node.id, NodeId(0));
        assert_eq!(node.properties.len(), 3);
        assert_eq!(node.properties[0], PropId(0));
        assert_eq!(node.properties[1], PropId(1));
        assert_eq!(node.properties[2], PropId(2));

        assert_eq!(g.properties[0].name, "subject.common_name");
        assert_eq!(g.properties[0].trust, TrustClass::Always);
        assert_eq!(g.properties[1].name, "subject.org");
        assert_eq!(g.properties[2].name, "public_key");
    }

    // -----------------------------------------------------------------------
    // Test: PKI example — nodes, properties, links, constraints
    // -----------------------------------------------------------------------

    fn make_pki_ast() -> AstGraph {
        AstGraph {
            domains: vec![],
            nodes: vec![
                // ca is a root (no parent link).
                ast_node_root(
                    "ca",
                    vec![
                        ("subject.common_name", AstTrustAnnotation::Always),
                        ("subject.org", AstTrustAnnotation::Always),
                        ("public_key", AstTrustAnnotation::Always),
                    ],
                    true,
                ),
                ast_node(
                    "cert",
                    vec![
                        ("issuer.common_name", AstTrustAnnotation::Default), // Critical
                        ("issuer.org", AstTrustAnnotation::Default),         // Critical
                        ("subject.common_name", AstTrustAnnotation::Constrained),
                        ("subject.org", AstTrustAnnotation::Constrained),
                        ("public_key", AstTrustAnnotation::Default),    // Critical
                        ("signature", AstTrustAnnotation::Default),     // Critical
                    ],
                ),
                ast_node(
                    "tls",
                    vec![
                        ("server_cert", AstTrustAnnotation::Default),   // Critical
                        ("cipher_suite", AstTrustAnnotation::Constrained),
                    ],
                ),
                // revocation is also a root (no parent link).
                ast_node_root(
                    "revocation",
                    vec![("crl", AstTrustAnnotation::Always)],
                    true,
                ),
            ],
            links: vec![
                AstLink {
                    child_ident: "cert".to_string(),
                    parent_ident: "ca".to_string(),
                    operation: Some("sign".to_string()),
                },
                AstLink {
                    child_ident: "tls".to_string(),
                    parent_ident: "cert".to_string(),
                    operation: None,
                },
            ],
            constraints: vec![
                // ca::subject.common_name -> cert::issuer.common_name
                AstConstraint {
                    dest_node: "cert".to_string(),
                    dest_prop: "issuer.common_name".to_string(),
                    source: prop_ref("ca", "subject.common_name"),
                    operation: Some("equality".to_string()),
                },
                // ca::subject.org -> cert::issuer.org
                AstConstraint {
                    dest_node: "cert".to_string(),
                    dest_prop: "issuer.org".to_string(),
                    source: prop_ref("ca", "subject.org"),
                    operation: Some("equality".to_string()),
                },
                // ca::public_key -> cert::signature
                AstConstraint {
                    dest_node: "cert".to_string(),
                    dest_prop: "signature".to_string(),
                    source: prop_ref("ca", "public_key"),
                    operation: Some("verified_by".to_string()),
                },
                // revocation::crl -> cert::subject.common_name
                AstConstraint {
                    dest_node: "cert".to_string(),
                    dest_prop: "subject.common_name".to_string(),
                    source: prop_ref("revocation", "crl"),
                    operation: Some("not_in".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_pki_node_count() {
        let g = build(make_pki_ast()).expect("PKI graph should build");
        assert_eq!(g.nodes.len(), 4);
    }

    #[test]
    fn test_pki_property_ids() {
        let g = build(make_pki_ast()).expect("PKI graph should build");

        // P0..P2: ca
        assert_eq!(g.properties[0].name, "subject.common_name");
        assert_eq!(g.properties[0].node, NodeId(0)); // ca
        assert_eq!(g.properties[0].trust, TrustClass::Always);

        assert_eq!(g.properties[1].name, "subject.org");
        assert_eq!(g.properties[2].name, "public_key");

        // P3..P8: cert
        assert_eq!(g.properties[3].name, "issuer.common_name");
        assert_eq!(g.properties[3].node, NodeId(1)); // cert
        assert_eq!(g.properties[3].trust, TrustClass::Critical);

        assert_eq!(g.properties[4].name, "issuer.org");
        assert_eq!(g.properties[4].trust, TrustClass::Critical);

        assert_eq!(g.properties[5].name, "subject.common_name");
        assert_eq!(g.properties[5].trust, TrustClass::Constrained);

        assert_eq!(g.properties[6].name, "subject.org");
        assert_eq!(g.properties[6].trust, TrustClass::Constrained);

        assert_eq!(g.properties[7].name, "public_key");
        assert_eq!(g.properties[7].trust, TrustClass::Critical);

        assert_eq!(g.properties[8].name, "signature");
        assert_eq!(g.properties[8].trust, TrustClass::Critical);

        // P9..P10: tls
        assert_eq!(g.properties[9].name, "server_cert");
        assert_eq!(g.properties[9].node, NodeId(2)); // tls
        assert_eq!(g.properties[9].trust, TrustClass::Critical);

        assert_eq!(g.properties[10].name, "cipher_suite");
        assert_eq!(g.properties[10].trust, TrustClass::Constrained);

        // P11: revocation
        assert_eq!(g.properties[11].name, "crl");
        assert_eq!(g.properties[11].node, NodeId(3)); // revocation
        assert_eq!(g.properties[11].trust, TrustClass::Always);

        assert_eq!(g.properties.len(), 12);
    }

    #[test]
    fn test_pki_links() {
        let g = build(make_pki_ast()).expect("PKI graph should build");

        // E0: ca -> cert (sign)
        // E1: cert -> tls
        let link_edges: Vec<_> = g
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_link())
            .collect();
        assert_eq!(link_edges.len(), 2);

        match &link_edges[0].1 {
            Edge::Link {
                child,
                parent,
                operation,
            } => {
                assert_eq!(*child, NodeId(1));  // cert
                assert_eq!(*parent, NodeId(0)); // ca
                assert_eq!(operation.as_deref(), Some("sign"));
            }
            _ => panic!("expected Link"),
        }

        match &link_edges[1].1 {
            Edge::Link {
                child,
                parent,
                operation,
            } => {
                assert_eq!(*child, NodeId(2));  // tls
                assert_eq!(*parent, NodeId(1)); // cert
                assert!(operation.is_none());
            }
            _ => panic!("expected Link"),
        }
    }

    #[test]
    fn test_pki_constraints() {
        let g = build(make_pki_ast()).expect("PKI graph should build");

        let constraint_edges: Vec<_> = g
            .edges
            .iter()
            .filter(|e| e.is_constraint())
            .collect();
        assert_eq!(constraint_edges.len(), 4);

        // E2: P0 -> P3 (equality)
        match &constraint_edges[0] {
            Edge::Constraint {
                dest_prop,
                source_prop,
                operation,
            } => {
                assert_eq!(*source_prop, PropId(0)); // ca::subject.common_name
                assert_eq!(*dest_prop, PropId(3));   // cert::issuer.common_name
                assert_eq!(operation.as_deref(), Some("equality"));
            }
            _ => panic!("expected Constraint"),
        }

        // E5: P11 -> P5 (not_in)
        match &constraint_edges[3] {
            Edge::Constraint {
                dest_prop,
                source_prop,
                operation,
            } => {
                assert_eq!(*source_prop, PropId(11)); // revocation::crl
                assert_eq!(*dest_prop, PropId(5));    // cert::subject.common_name
                assert_eq!(operation.as_deref(), Some("not_in"));
            }
            _ => panic!("expected Constraint"),
        }
    }

    #[test]
    fn test_pki_adjacency() {
        let g = build(make_pki_ast()).expect("PKI graph should build");

        // ca (NodeId(0)) should have one child link: cert
        let ca_children = g.children_of(NodeId(0));
        assert_eq!(ca_children.len(), 1);

        // cert (NodeId(1)) should have one parent (ca) and one child (tls)
        assert!(g.node_parent.contains_key(&NodeId(1)));
        let cert_children = g.children_of(NodeId(1));
        assert_eq!(cert_children.len(), 1);

        // tls (NodeId(2)) should have a parent (cert) and no children
        assert!(g.node_parent.contains_key(&NodeId(2)));
        assert_eq!(g.children_of(NodeId(2)).len(), 0);

        // P0 (ca::subject.common_name) is involved in one constraint edge.
        let p0_edges = g.edges_on_prop(PropId(0));
        assert_eq!(p0_edges.len(), 1);
        assert!(g.edges[p0_edges[0].index()].is_constraint());
    }

    // -----------------------------------------------------------------------
    // Test: domains
    // -----------------------------------------------------------------------

    #[test]
    fn test_domains() {
        // All nodes are roots since there are no links in this test.
        let ast = AstGraph {
            domains: vec![AstDomain {
                display_name: "Infra".to_string(),
                nodes: vec![
                    ast_node_root("alpha", vec![("x", AstTrustAnnotation::Always)], true),
                    ast_node_root("beta", vec![("y", AstTrustAnnotation::Default)], true),
                ],
            }],
            nodes: vec![ast_node_root("gamma", vec![("z", AstTrustAnnotation::Constrained)], true)],
            links: vec![],
            constraints: vec![],
        };
        let g = build(ast).expect("domain graph should build");

        // Domain nodes come first, then top-level nodes.
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.nodes[0].ident, "alpha");
        assert_eq!(g.nodes[0].id, NodeId(0));
        assert_eq!(g.nodes[0].domain, Some(DomainId(0)));
        assert_eq!(g.nodes[1].ident, "beta");
        assert_eq!(g.nodes[1].domain, Some(DomainId(0)));
        assert_eq!(g.nodes[2].ident, "gamma");
        assert_eq!(g.nodes[2].domain, None);

        assert_eq!(g.domains.len(), 1);
        assert_eq!(g.domains[0].display_name, "Infra");
        assert_eq!(g.domains[0].members, vec![NodeId(0), NodeId(1)]);

        // Properties: P0=alpha::x, P1=beta::y, P2=gamma::z
        assert_eq!(g.properties.len(), 3);
        assert_eq!(g.properties[0].name, "x");
        assert_eq!(g.properties[1].name, "y");
        assert_eq!(g.properties[2].name, "z");
    }

    // -----------------------------------------------------------------------
    // Test: derivation expressions
    // -----------------------------------------------------------------------

    #[test]
    fn test_derivation_simple() {
        // Two nodes: signer (public_key @always) and verifier (sig @critical).
        // signer is a root; verifier is a child of signer via Link.
        // Constraint: verifier::sig <= verify(signer::public_key) : verified_by
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                ast_node_root("signer", vec![("public_key", AstTrustAnnotation::Always)], true),
                ast_node("verifier", vec![("sig", AstTrustAnnotation::Default)]),
            ],
            links: vec![AstLink {
                child_ident: "verifier".to_string(),
                parent_ident: "signer".to_string(),
                operation: None,
            }],
            constraints: vec![AstConstraint {
                dest_node: "verifier".to_string(),
                dest_prop: "sig".to_string(),
                source: AstSourceExpr::Derivation(AstDerivationExpr {
                    function: "verify".to_string(),
                    args: vec![prop_ref("signer", "public_key")],
                }),
                operation: Some("verified_by".to_string()),
            }],
        };

        let g = build(ast).expect("derivation graph should build");

        // One derivation should be created.
        assert_eq!(g.derivations.len(), 1);
        let deriv = &g.derivations[0];
        assert_eq!(deriv.id, DerivId(0));
        assert_eq!(deriv.operation, "verify");
        assert_eq!(deriv.inputs.len(), 1);
        assert_eq!(deriv.inputs[0], PropId(0)); // signer::public_key

        // The output_prop is an ephemeral property beyond the declared ones.
        // Declared: P0=signer::public_key, P1=verifier::sig
        // Ephemeral output: P2
        assert_eq!(deriv.output_prop, PropId(2));

        // Edges: E0=Link(signer->verifier), E1=DerivInput(P0->D0), E2=Constraint(P2->P1)
        assert_eq!(g.edges.len(), 3);
        assert!(g.edges[0].is_link());
        assert!(g.edges[1].is_deriv_input());
        assert!(g.edges[2].is_constraint());

        match &g.edges[1] {
            Edge::DerivInput {
                source_prop,
                target_deriv,
            } => {
                assert_eq!(*source_prop, PropId(0));
                assert_eq!(*target_deriv, DerivId(0));
            }
            _ => panic!("expected DerivInput"),
        }

        match &g.edges[2] {
            Edge::Constraint {
                dest_prop,
                source_prop,
                operation,
            } => {
                assert_eq!(*dest_prop, PropId(1));   // verifier::sig
                assert_eq!(*source_prop, PropId(2)); // deriv output
                assert_eq!(operation.as_deref(), Some("verified_by"));
            }
            _ => panic!("expected Constraint"),
        }
    }

    // -----------------------------------------------------------------------
    // Test: derivation deduplication
    // -----------------------------------------------------------------------

    #[test]
    fn test_derivation_deduplication() {
        // Two constraints with identical derivation expressions.
        // Both should resolve to the same DerivId and output_prop.
        let deriv_expr = AstSourceExpr::Derivation(AstDerivationExpr {
            function: "hash".to_string(),
            args: vec![prop_ref("src", "data")],
        });

        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                // src is a root; dst is a child of src via Link.
                ast_node_root("src", vec![("data", AstTrustAnnotation::Always)], true),
                ast_node(
                    "dst",
                    vec![
                        ("hash_a", AstTrustAnnotation::Default),
                        ("hash_b", AstTrustAnnotation::Default),
                    ],
                ),
            ],
            links: vec![AstLink {
                child_ident: "dst".to_string(),
                parent_ident: "src".to_string(),
                operation: None,
            }],
            constraints: vec![
                AstConstraint {
                    dest_node: "dst".to_string(),
                    dest_prop: "hash_a".to_string(),
                    source: deriv_expr.clone(),
                    operation: None,
                },
                AstConstraint {
                    dest_node: "dst".to_string(),
                    dest_prop: "hash_b".to_string(),
                    source: deriv_expr.clone(),
                    operation: None,
                },
            ],
        };

        let g = build(ast).expect("dedup graph should build");

        // Only one derivation should exist.
        assert_eq!(g.derivations.len(), 1);

        // Both constraint edges should reference the same source_prop (the
        // derivation's output_prop).
        let constraint_edges: Vec<_> = g
            .edges
            .iter()
            .filter(|e| e.is_constraint())
            .collect();
        assert_eq!(constraint_edges.len(), 2);

        let src0 = match &constraint_edges[0] {
            Edge::Constraint { source_prop, .. } => *source_prop,
            _ => panic!(),
        };
        let src1 = match &constraint_edges[1] {
            Edge::Constraint { source_prop, .. } => *source_prop,
            _ => panic!(),
        };

        // Both constraints share the same derivation output prop.
        assert_eq!(src0, src1);
        assert_eq!(src0, g.derivations[0].output_prop);
    }

    // -----------------------------------------------------------------------
    // Test: duplicate node ident is rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_duplicate_node_rejected() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                ast_node("foo", vec![]),
                ast_node("foo", vec![]),
            ],
            links: vec![],
            constraints: vec![],
        };
        assert!(build(ast).is_err());
    }

    // -----------------------------------------------------------------------
    // Test: unknown node in link is rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_unknown_node_in_link_rejected() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![ast_node_root("known", vec![], false)],
            links: vec![AstLink {
                child_ident: "known".to_string(),
                parent_ident: "unknown".to_string(), // "unknown" doesn't exist
                operation: None,
            }],
            constraints: vec![],
        };
        assert!(build(ast).is_err());
    }

    // -----------------------------------------------------------------------
    // Test: unknown prop in constraint is rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_unknown_prop_in_constraint_rejected() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                ast_node_root("a", vec![("x", AstTrustAnnotation::Always)], true),
                ast_node("b", vec![("y", AstTrustAnnotation::Default)]),
            ],
            links: vec![AstLink {
                child_ident: "b".to_string(),
                parent_ident: "a".to_string(),
                operation: None,
            }],
            constraints: vec![AstConstraint {
                dest_node: "b".to_string(),
                dest_prop: "y".to_string(),
                source: prop_ref("a", "NONEXISTENT"),
                operation: None,
            }],
        };
        assert!(build(ast).is_err());
    }

    // -----------------------------------------------------------------------
    // Test: multi-parent link is rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_multi_parent_rejected() {
        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                ast_node_root("p1", vec![], true),
                ast_node_root("p2", vec![], true),
                ast_node("child", vec![]),
            ],
            links: vec![
                AstLink {
                    child_ident: "child".to_string(),
                    parent_ident: "p1".to_string(),
                    operation: None,
                },
                AstLink {
                    child_ident: "child".to_string(),
                    parent_ident: "p2".to_string(),
                    operation: None,
                },
            ],
            constraints: vec![],
        };
        assert!(build(ast).is_err());
    }

    // -----------------------------------------------------------------------
    // Test: nested derivation
    // -----------------------------------------------------------------------

    #[test]
    fn test_nested_derivation() {
        // verifier::out <= outer(inner(src::x))
        let inner = AstSourceExpr::Derivation(AstDerivationExpr {
            function: "inner".to_string(),
            args: vec![prop_ref("src", "x")],
        });
        let outer = AstSourceExpr::Derivation(AstDerivationExpr {
            function: "outer".to_string(),
            args: vec![inner],
        });

        let ast = AstGraph {
            domains: vec![],
            nodes: vec![
                // src is root; verifier is its child.
                ast_node_root("src", vec![("x", AstTrustAnnotation::Always)], true),
                ast_node("verifier", vec![("out", AstTrustAnnotation::Default)]),
            ],
            links: vec![AstLink {
                child_ident: "verifier".to_string(),
                parent_ident: "src".to_string(),
                operation: None,
            }],
            constraints: vec![AstConstraint {
                dest_node: "verifier".to_string(),
                dest_prop: "out".to_string(),
                source: outer,
                operation: None,
            }],
        };

        let g = build(ast).expect("nested derivation should build");

        // Should create two derivations: inner and outer.
        assert_eq!(g.derivations.len(), 2);

        // inner derivation takes src::x (P0) as input.
        let inner_deriv = g
            .derivations
            .iter()
            .find(|d| d.operation == "inner")
            .expect("inner derivation");
        assert_eq!(inner_deriv.inputs, vec![PropId(0)]);

        // outer derivation takes inner's output as input.
        let outer_deriv = g
            .derivations
            .iter()
            .find(|d| d.operation == "outer")
            .expect("outer derivation");
        assert_eq!(outer_deriv.inputs, vec![inner_deriv.output_prop]);
    }
}
