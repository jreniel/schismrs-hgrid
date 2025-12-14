use super::nodes::Nodes;
use derive_builder::Builder;
use linked_hash_map::LinkedHashMap;
use std::sync::Arc;

/// Open boundary segments for an unstructured mesh.
/// Validation is performed separately via `Hgrid::check_validity()`.
#[derive(Builder, Debug, Clone)]
#[allow(dead_code)]
pub struct OpenBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl OpenBoundaries {
    pub fn nodes_ids(&self) -> &Vec<Vec<u32>> {
        &self.nodes_ids
    }
    pub fn iter(&self) -> OpenBoundariesIter<'_> {
        OpenBoundariesIter {
            outer: &self.nodes_ids,
            current: 0,
        }
    }
}

pub struct OpenBoundariesIter<'a> {
    outer: &'a Vec<Vec<u32>>,
    current: usize,
}

impl<'a> Iterator for OpenBoundariesIter<'a> {
    type Item = &'a Vec<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.outer.len() {
            let item = &self.outer[self.current];
            self.current += 1;
            Some(item)
        } else {
            None
        }
    }
}

/// Land boundary segments for an unstructured mesh.
/// Validation is performed separately via `Hgrid::check_validity()`.
#[derive(Builder, Debug, Clone)]
#[allow(dead_code)]
pub struct LandBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl LandBoundaries {
    pub fn nodes_ids(&self) -> &Vec<Vec<u32>> {
        &self.nodes_ids
    }
}

/// Interior boundary segments for an unstructured mesh.
/// Validation is performed separately via `Hgrid::check_validity()`.
#[derive(Builder, Debug, Clone)]
#[allow(dead_code)]
pub struct InteriorBoundaries {
    nodes: Arc<Nodes>,
    nodes_ids: Vec<Vec<u32>>,
}

impl InteriorBoundaries {
    pub fn nodes_ids(&self) -> &Vec<Vec<u32>> {
        &self.nodes_ids
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Boundaries {
    #[builder(default)]
    open: Option<OpenBoundaries>,
    #[builder(default)]
    land: Option<LandBoundaries>,
    #[builder(default)]
    interior: Option<InteriorBoundaries>,
}

impl Boundaries {
    pub fn to_boundary_type_map(&self) -> LinkedHashMap<BoundaryType, &Vec<Vec<u32>>> {
        let mut map = LinkedHashMap::new();

        if let Some(ref open_boundary) = self.open {
            map.insert(BoundaryType::Open, open_boundary.nodes_ids());
        }

        if let Some(ref land_boundary) = self.land {
            map.insert(BoundaryType::Land, land_boundary.nodes_ids());
        }

        if let Some(ref interior_boundary) = self.interior {
            map.insert(BoundaryType::Interior, interior_boundary.nodes_ids());
        }

        map
    }
    pub fn open(&self) -> Option<&OpenBoundaries> {
        self.open.as_ref()
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum BoundaryType {
    Open,
    Land,
    Interior,
}
