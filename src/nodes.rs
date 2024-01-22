use derive_builder::Builder;
use ndarray::prelude::*;
use proj::Proj;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Nodes {
    btree_map: BTreeMap<u32, (Vec<f64>, Option<Vec<f64>>)>,
    crs: Option<Arc<Proj>>,
}

impl Nodes {
    pub fn btree_map(&self) -> BTreeMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        self.btree_map.clone()
    }

    pub fn crs(&self) -> Option<Arc<Proj>> {
        self.crs.clone()
    }

    // pub fn new(btree_map: BTreeMap<u32, (Vec<f64>, Option<Vec<f64>>)>, crs: Option<Proj>) -> Self {
    //     let crs = crs.map(Arc::new);
    //     Self { btree_map, crs }
    // }

    pub fn x(&self) -> Array1<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.btree_map {
            v.push(coords[0]);
        }
        Array1::from(v)
    }

    pub fn y(&self) -> Array1<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.btree_map {
            v.push(coords[1]);
        }
        Array1::from(v)
    }

    pub fn xy(&self) -> Array2<f64> {
        let mut v = Vec::new();
        for (_node_id, (coords, _values)) in &self.btree_map {
            v.push(coords[0]);
            v.push(coords[1]);
        }
        Array2::from_shape_vec((self.len(), 2), v).unwrap()
    }

    pub fn len(&self) -> usize {
        self.btree_map.len()
    }

    // pub fn set_crs(&mut self, crs: Option<Proj>) {
    //     let crs = Rc::new(crs.unwrap());
    //     self.crs = Some(crs);
    // }
}
