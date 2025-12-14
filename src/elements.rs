use super::nodes::Nodes;
use derive_builder::Builder;
use linked_hash_map::LinkedHashMap;
use std::sync::Arc;

/// Element connectivity data for an unstructured mesh.
///
/// Only data format validation (element sizes) is performed during construction.
/// Full structural and geometric validation is available via `Hgrid::check_validity()`.
#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct Elements {
    hash_map: LinkedHashMap<u32, Vec<u32>>,
    nodes: Arc<Nodes>,
}

impl ElementsBuilder {
    /// Validates that all elements have exactly 3 or 4 nodes (triangles or quads).
    /// This is a fast data format check - structural/geometric validation is separate.
    fn validate(&self) -> Result<(), ElementsBuilderError> {
        if let Some(hash_map) = &self.hash_map {
            for (elem_id, nodes) in hash_map.iter() {
                let len = nodes.len();
                if len != 3 && len != 4 {
                    return Err(ElementsBuilderError::ValidationError(format!(
                        "Element {} has {} nodes (expected 3 or 4)",
                        elem_id, len
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Elements {
    pub fn hash_map(&self) -> &LinkedHashMap<u32, Vec<u32>> {
        &self.hash_map
    }
    pub fn nodes(&self) -> &Nodes {
        &self.nodes
    }
}

// #[derive(Error, Debug, Clone)]
// pub enum ElementsConstructorError {
//     #[error("Element set is not a subset of node set")]
//     InvalidSubset,

//     #[error("Invalid number of nodes in an element")]
//     InvalidElementSize,
// }
