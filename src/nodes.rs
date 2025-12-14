use derive_builder::Builder;
use ndarray::prelude::*;
use proj::Proj;
// use std::collections::BTreeMap;
use linked_hash_map::LinkedHashMap;
use std::sync::Arc;

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Nodes {
    hash_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)>,
    #[builder(default)]
    crs: Option<Arc<Proj>>,
}

impl Nodes {
    pub fn hash_map(&self) -> &LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        &self.hash_map
    }

    pub fn crs(&self) -> Option<Arc<Proj>> {
        self.crs.clone()
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
