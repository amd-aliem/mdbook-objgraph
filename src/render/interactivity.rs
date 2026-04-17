//! Inline JavaScript for hover/click visibility toggling (DESIGN.md §5.4).

/// Returns the complete inline JavaScript for an obgraph SVG.
pub fn js() -> &'static str {
    r#"(function() {
  var svg = document.currentScript.closest('.obgraph');
  if (!svg) return;

  var selected = new Set();
  var hovered = new Set();
  var hoveredProp = null;
  var selectedProps = new Set();

  function hasSelectedParticipant(el) {
    var attr = el.getAttribute('data-participants');
    if (!attr) return false;
    var ids = attr.split(',');
    for (var i = 0; i < ids.length; i++) {
      if (selected.has(ids[i])) return true;
    }
    return false;
  }

  function hasHoveredParticipant(el) {
    var attr = el.getAttribute('data-participants');
    if (!attr) return false;
    var ids = attr.split(',');
    for (var i = 0; i < ids.length; i++) {
      if (hovered.has(ids[i])) return true;
    }
    return false;
  }

  function matchesHoveredProp(el) {
    if (!hoveredProp) return false;
    var attr = el.getAttribute('data-props');
    if (!attr) return false;
    var ids = attr.split(',');
    for (var i = 0; i < ids.length; i++) {
      if (ids[i] === hoveredProp) return true;
    }
    return false;
  }

  function hasSelectedProp(el) {
    if (selectedProps.size === 0) return false;
    var attr = el.getAttribute('data-props');
    if (!attr) return false;
    var ids = attr.split(',');
    for (var i = 0; i < ids.length; i++) {
      if (selectedProps.has(ids[i])) return true;
    }
    return false;
  }

  // An edge/label should be visible if:
  //   (a) any participant node is selected or hovered
  //       AND (no prop hover, or prop matches), OR
  //   (b) any prop is in selectedProps
  function isEdgeVisible(el) {
    if (hasSelectedProp(el)) return true;
    if (hasSelectedParticipant(el)) {
      if (hoveredProp === null) return true;
      return matchesHoveredProp(el);
    }
    if (!hasHoveredParticipant(el)) return false;
    if (hoveredProp === null) return true;
    return matchesHoveredProp(el);
  }

  function updateEdges() {
    svg.querySelectorAll('.obgraph-constraint-full').forEach(function(p) {
      if (isEdgeVisible(p)) {
        p.classList.add('obgraph-active');
      } else {
        p.classList.remove('obgraph-active');
      }
    });
    svg.querySelectorAll('.obgraph-constraint-stub').forEach(function(p) {
      if (isEdgeVisible(p)) {
        p.classList.add('obgraph-hidden');
      } else {
        p.classList.remove('obgraph-hidden');
      }
    });
    svg.querySelectorAll('.obgraph-edge-label').forEach(function(g) {
      if (isEdgeVisible(g)) {
        g.classList.add('obgraph-label-visible');
      } else {
        g.classList.remove('obgraph-label-visible');
      }
    });
  }

  svg.querySelectorAll('.obgraph-node').forEach(function(node) {
    var id = node.getAttribute('data-node');
    if (node.getAttribute('data-selected') === 'true') {
      selected.add(id);
    }
    node.addEventListener('mouseenter', function() {
      hovered.add(id);
      updateEdges();
    });
    node.addEventListener('mouseleave', function() {
      hovered.delete(id);
      hoveredProp = null;
      updateEdges();
    });
    node.querySelectorAll('.obgraph-prop').forEach(function(prop) {
      prop.addEventListener('mouseenter', function() {
        hoveredProp = prop.getAttribute('data-prop');
        updateEdges();
      });
      prop.addEventListener('mouseleave', function() {
        hoveredProp = null;
        updateEdges();
      });
      prop.addEventListener('click', function(e) {
        e.stopPropagation();
        var pid = prop.getAttribute('data-prop');
        if (selectedProps.has(pid)) {
          selectedProps.delete(pid);
          prop.removeAttribute('data-prop-selected');
        } else {
          selectedProps.add(pid);
          prop.setAttribute('data-prop-selected', 'true');
        }
        updateEdges();
      });
    });
    node.addEventListener('click', function(e) {
      e.stopPropagation();
      if (selected.has(id)) {
        selected.delete(id);
        node.setAttribute('data-selected', 'false');
      } else {
        selected.add(id);
        node.setAttribute('data-selected', 'true');
      }
      updateEdges();
    });
  });

  svg.addEventListener('click', function() {
    selected.clear();
    selectedProps.clear();
    svg.querySelectorAll('.obgraph-node').forEach(function(node) {
      node.setAttribute('data-selected', 'false');
    });
    svg.querySelectorAll('[data-prop-selected]').forEach(function(el) {
      el.removeAttribute('data-prop-selected');
    });
    updateEdges();
  });

  updateEdges();
})();"#
}
