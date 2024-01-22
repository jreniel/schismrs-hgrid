use super::nodes::Nodes;
use derive_builder::Builder;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
// use thiserror::Error;

#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct Elements {
    btree_map: BTreeMap<u32, Vec<u32>>,
    nodes: Arc<Nodes>,
}

impl ElementsBuilder {
    pub fn validate(&self) -> Result<(), ElementsBuilderError> {
        let element_hash_set: HashSet<u32> =
            self.btree_map.as_ref().map_or(HashSet::new(), |btree_map| {
                btree_map
                    .values()
                    .flat_map(|vec| vec.iter())
                    .cloned()
                    .collect()
            });

        let node_hash_set: HashSet<u32> = self.nodes.as_ref().map_or(HashSet::new(), |nodes_arc| {
            let nodes = Arc::as_ref(nodes_arc);
            nodes.btree_map().keys().cloned().collect()
        });

        if !element_hash_set.is_subset(&node_hash_set) {
            return Err(ElementsBuilderError::ValidationError(
                "Some elements are not a subset of node_hash_set".to_string(),
            ));
        }

        if let Some(btree_map) = &self.btree_map {
            let valid_lengths = btree_map
                .values()
                .all(|vec| vec.len() == 3 || vec.len() == 4);
            if !valid_lengths {
                return Err(ElementsBuilderError::ValidationError(
                    "All members of btree_map must have a length of 3 or 4".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl Elements {
    pub fn btree_map(&self) -> BTreeMap<u32, Vec<u32>> {
        self.btree_map.clone()
    }
}

// #[derive(Error, Debug, Clone)]
// pub enum ElementsConstructorError {
//     #[error("Element set is not a subset of node set")]
//     InvalidSubset,

//     #[error("Invalid number of nodes in an element")]
//     InvalidElementSize,
// }
