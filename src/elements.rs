use super::nodes::Nodes;
use derive_builder::Builder;
use linked_hash_map::LinkedHashMap;
use std::collections::HashSet;
use std::sync::Arc;
// use thiserror::Error;

#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct Elements {
    hash_map: LinkedHashMap<u32, Vec<u32>>,
    nodes: Arc<Nodes>,
}

impl ElementsBuilder {
    pub fn validate(&self) -> Result<(), ElementsBuilderError> {
        let element_hash_set: HashSet<u32> =
            self.hash_map.as_ref().map_or(HashSet::new(), |hash_map| {
                hash_map
                    .values()
                    .flat_map(|vec| vec.iter())
                    .cloned()
                    .collect()
            });

        let node_hash_set: HashSet<u32> = self.nodes.as_ref().map_or(HashSet::new(), |nodes_arc| {
            let nodes = Arc::as_ref(nodes_arc);
            nodes.hash_map().keys().cloned().collect()
        });

        if !element_hash_set.is_subset(&node_hash_set) {
            return Err(ElementsBuilderError::ValidationError(
                "Some elements are not a subset of node_hash_set".to_string(),
            ));
        }

        if let Some(hash_map) = &self.hash_map {
            let valid_lengths = hash_map
                .values()
                .all(|vec| vec.len() == 3 || vec.len() == 4);
            if !valid_lengths {
                return Err(ElementsBuilderError::ValidationError(
                    "All members of hash_map must have a length of 3 or 4".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl Elements {
    pub fn hash_map(&self) -> &LinkedHashMap<u32, Vec<u32>> {
        &self.hash_map
    }
}

// #[derive(Error, Debug, Clone)]
// pub enum ElementsConstructorError {
//     #[error("Element set is not a subset of node set")]
//     InvalidSubset,

//     #[error("Invalid number of nodes in an element")]
//     InvalidElementSize,
// }
