use super::nodes::Nodes;
use derive_builder::Builder;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct OpenBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl OpenBoundaries {
    pub fn nodes_ids(&self) -> Vec<Vec<u32>> {
        self.nodes_ids.clone()
    }
}

impl OpenBoundariesBuilder {
    pub fn validate(&self) -> Result<(), OpenBoundariesBuilderError> {
        let node_hash_set: HashSet<u32> = self.nodes.as_ref().map_or(HashSet::new(), |nodes_arc| {
            let nodes = Arc::as_ref(nodes_arc);
            nodes.btree_map().keys().cloned().collect()
        });

        let all_node_ids: HashSet<u32> =
            self.nodes_ids.as_ref().map_or(HashSet::new(), |node_ids| {
                node_ids
                    .iter()
                    .flat_map(|ids| ids.iter())
                    .cloned()
                    .collect()
            });
        if !all_node_ids.is_subset(&node_hash_set) {
            return Err(OpenBoundariesBuilderError::ValidationError(
                "Found open boundary node ids not in nodes.".to_string(),
            ));
        };
        Ok(())
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct LandBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl LandBoundariesBuilder {
    pub fn validate(&self) -> Result<(), LandBoundariesBuilderError> {
        let node_hash_set: HashSet<u32> = self.nodes.as_ref().map_or(HashSet::new(), |nodes_arc| {
            let nodes = Arc::as_ref(nodes_arc);
            nodes.btree_map().keys().cloned().collect()
        });

        let all_node_ids: HashSet<u32> =
            self.nodes_ids.as_ref().map_or(HashSet::new(), |node_ids| {
                node_ids
                    .iter()
                    .flat_map(|ids| ids.iter())
                    .cloned()
                    .collect()
            });
        if !all_node_ids.is_subset(&node_hash_set) {
            return Err(LandBoundariesBuilderError::ValidationError(
                "Found land boundary node ids not in nodes.".to_string(),
            ));
        };
        Ok(())
    }
}

impl LandBoundaries {
    pub fn nodes_ids(&self) -> Vec<Vec<u32>> {
        self.nodes_ids.clone()
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct InteriorBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl InteriorBoundariesBuilder {
    pub fn validate(&self) -> Result<(), InteriorBoundariesBuilderError> {
        let node_hash_set: HashSet<u32> = self.nodes.as_ref().map_or(HashSet::new(), |nodes_arc| {
            let nodes = Arc::as_ref(nodes_arc);
            nodes.btree_map().keys().cloned().collect()
        });

        let all_node_ids: HashSet<u32> =
            self.nodes_ids.as_ref().map_or(HashSet::new(), |node_ids| {
                node_ids
                    .iter()
                    .flat_map(|ids| ids.iter())
                    .cloned()
                    .collect()
            });
        if !all_node_ids.is_subset(&node_hash_set) {
            return Err(InteriorBoundariesBuilderError::ValidationError(
                "Found interior boundary node ids not in nodes.".to_string(),
            ));
        };
        Ok(())
    }
}
impl InteriorBoundaries {
    pub fn nodes_ids(&self) -> Vec<Vec<u32>> {
        self.nodes_ids.clone()
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Boundaries {
    open: Option<OpenBoundaries>,
    land: Option<LandBoundaries>,
    interior: Option<InteriorBoundaries>,
}

impl Boundaries {
    pub fn to_boundary_type_map(&self) -> BTreeMap<BoundaryType, Vec<Vec<u32>>> {
        let mut btree_map = BTreeMap::new();

        if let Some(ref open_boundary) = self.open {
            btree_map.insert(BoundaryType::Open, open_boundary.nodes_ids());
        }

        if let Some(ref land_boundary) = self.land {
            btree_map.insert(BoundaryType::Land, land_boundary.nodes_ids());
        }

        if let Some(ref interior_boundary) = self.interior {
            btree_map.insert(BoundaryType::Interior, interior_boundary.nodes_ids());
        }

        btree_map
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum BoundaryType {
    Open,
    Land,
    Interior,
}
