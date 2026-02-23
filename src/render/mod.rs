pub mod interactivity;
pub mod style;
pub mod svg;

use crate::layout::LayoutResult;
use crate::model::state::StateResult;
use crate::model::types::Graph;

/// Render a laid-out graph into a self-contained HTML/SVG fragment.
pub fn render(graph: &Graph, layout: &LayoutResult, state: &StateResult) -> String {
    svg::generate_svg(graph, layout, state)
}
