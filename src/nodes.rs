use derive_builder::Builder;
use ndarray::prelude::*;
use proj::Proj;
// use std::collections::BTreeMap;
use linked_hash_map::LinkedHashMap;

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Nodes {
    hash_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)>,
    /// CRS definition string (e.g., "EPSG:4326", "EPSG:32618")
    /// Stored as string to make Nodes Send + Sync safe.
    /// Use `proj()` to create a Proj instance when needed.
    #[builder(default)]
    crs: Option<String>,
}

impl Nodes {
    pub fn hash_map(&self) -> &LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        &self.hash_map
    }

    /// Get the CRS definition string
    pub fn crs(&self) -> Option<&str> {
        self.crs.as_deref()
    }

    /// Create a Proj instance from the CRS definition string.
    /// Returns None if no CRS is defined or if the CRS string is invalid.
    /// Each call creates a new Proj instance (thread-safe).
    pub fn proj(&self) -> Option<Proj> {
        self.crs.as_ref().and_then(|crs_str| Proj::new(crs_str).ok())
    }

    // pub fn new(hash_maptree_map, crs }
    // }

    pub fn x(&self) -> Array1<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.hash_map {
            v.push(coords[0]);
        }
        Array1::from(v)
    }

    pub fn y(&self) -> Array1<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.hash_map {
            v.push(coords[1]);
        }
        Array1::from(v)
    }

    pub fn xy(&self) -> Array2<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.hash_map {
            v.push(coords[0]);
            v.push(coords[1]);
        }
        Array2::from_shape_vec((self.len(), 2), v).unwrap()
    }

    pub fn len(&self) -> usize {
        self.hash_map.len()
    }

    // pub fn set_crs(&mut self, crs: Option<Proj>) {
    //     let crs = Rc::new(crs.unwrap());
    //     self.crs = Some(crs);
    // }
    pub fn get_node(&self, idx: u32) -> Option<(f64, f64)> {
        self.hash_map.get(&idx).map(|(coords, _)| {
            // Assuming coords[0] is longitude and coords[1] is latitude
            (coords[0], coords[1])
        })
    }
}
