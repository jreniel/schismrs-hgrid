use derive_builder::Builder;
use ndarray::prelude::*;
use proj::Proj;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Nodes {
    hash_map: HashMap<u32, (Vec<f64>, Option<Vec<f64>>)>,
    crs: Option<Arc<Proj>>,
}

impl Nodes {
    pub fn hash_map(&self) -> HashMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        self.hash_map.clone()
    }

    pub fn crs(&self) -> Option<Arc<Proj>> {
        self.crs.clone()
    }

    // pub fn new(hash_map: HashMap<u32, (Vec<f64>, Option<Vec<f64>>)>, crs: Option<Proj>) -> Self {
    //     let crs = crs.map(Arc::new);
    //     Self { hash_map, crs }
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
}
