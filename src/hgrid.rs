use super::gr3::{self, Gr3ParserOutputBuilder};
use super::{
    boundaries::{
        Boundaries, BoundariesBuilder, BoundariesBuilderError, BoundaryType,
        InteriorBoundariesBuilder, InteriorBoundariesBuilderError, LandBoundariesBuilder,
        LandBoundariesBuilderError, OpenBoundariesBuilder, OpenBoundariesBuilderError,
    },
    elements::{Elements, ElementsBuilder, ElementsBuilderError},
    gr3::{write_to_path, Gr3ParserOutput},
    nodes::{Nodes, NodesBuilder, NodesBuilderError},
};
use derive_builder::Builder;
use linked_hash_map::LinkedHashMap;
use ndarray::{Array1, Array2};
use proj::Proj;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use url::Url;

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Hgrid {
    nodes: Arc<Nodes>,
    elements: Elements,
    boundaries: Option<Boundaries>,
    description: Option<String>,
}

impl Hgrid {
    pub fn nodes(&self) -> &Nodes {
        &self.nodes
    }

    pub fn elements(&self) -> &Elements {
        &self.elements
    }

    pub fn boundaries(&self) -> Option<&Boundaries> {
        self.boundaries.as_ref()
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    pub fn x(&self) -> Array1<f64> {
        self.nodes.x()
    }

    pub fn y(&self) -> Array1<f64> {
        self.nodes.y()
    }

    pub fn depths(&self) -> Array1<f64> {
        let node_hashmap = self.nodes.hash_map();
        let depths: Vec<f64> = node_hashmap
            .values()
            .filter_map(|(_feats, depth_opt)| {
                depth_opt
                    .as_ref()
                    .and_then(|depths| depths.first().copied())
            })
            .collect();

        depths.into()
    }
    pub fn xy(&self) -> Array2<f64> {
        self.nodes.xy()
    }

    pub fn crs(&self) -> Option<Arc<Proj>> {
        self.nodes.crs()
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let mut gr3_parser_output_builder = Gr3ParserOutputBuilder::default();
        gr3_parser_output_builder.description(self.description.clone());
        // since gr3 reverses hgrid values...
        let reversed_nodes: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = self
            .nodes
            .hash_map()
            .iter()
            .map(|(&node_id, (coord, value))| {
                let reversed_value = value.as_ref().map(|v| v.iter().map(|&x| -x).collect());
                (node_id, (coord.clone(), reversed_value))
            })
            .collect();
        gr3_parser_output_builder.nodes(reversed_nodes);
        gr3_parser_output_builder.elements(self.elements.hash_map().clone());
        gr3_parser_output_builder.crs(self.crs().clone());
        if let Some(boundaries) = &self.boundaries {
            let the_type_map = boundaries.to_boundary_type_map();
            gr3_parser_output_builder.open_boundaries(the_type_map[&BoundaryType::Open].clone());
            gr3_parser_output_builder.land_boundaries(the_type_map[&BoundaryType::Land].clone());
            gr3_parser_output_builder
                .interior_boundaries(the_type_map[&BoundaryType::Interior].clone());
        } else {
            gr3_parser_output_builder.open_boundaries(Vec::new());
            gr3_parser_output_builder.land_boundaries(Vec::new());
            gr3_parser_output_builder.interior_boundaries(Vec::new());
        }
        let gr3_parser_output = gr3_parser_output_builder.build().unwrap();
        write_to_path(path, &gr3_parser_output)
    }

    pub fn get_number_of_elements_connected_to_each_node(&self) -> Array1<usize> {
        let mut counts = vec![0; self.nodes.len() + 1];
        for (_element, node_ids) in self.elements.hash_map().iter() {
            for node_id in node_ids {
                counts[*node_id as usize] += 1;
            }
        }
        Array1::from(counts)
    }
}

#[derive(Error, Debug)]
pub enum HgridTryFromError {
    #[error("Error loading from path: {0}, error: {1}")]
    TryFromPathBufError(String, String),

    #[error("Error loading from URL: {0}, error: {1}")]
    TryFromUrlError(String, String),

    #[error(transparent)]
    NodesBuilderError(#[from] NodesBuilderError),

    #[error(transparent)]
    ElementsBuilderError(#[from] ElementsBuilderError),

    #[error(transparent)]
    BoundariesBuilderError(#[from] BoundariesBuilderError),

    #[error(transparent)]
    OpenBoundariesBuilderError(#[from] OpenBoundariesBuilderError),

    #[error(transparent)]
    LandBoundariesBuilderError(#[from] LandBoundariesBuilderError),

    #[error(transparent)]
    InteriorBoundariesBuilderError(#[from] InteriorBoundariesBuilderError),
}

impl TryFrom<&PathBuf> for Hgrid {
    type Error = HgridTryFromError;
    fn try_from(path: &PathBuf) -> Result<Self, Self::Error> {
        let parsed_gr3 = gr3::parse_from_path_ref(&path).map_err(|e| {
            HgridTryFromError::TryFromPathBufError(path.display().to_string(), e.to_string())
        })?;
        Hgrid::try_from(&parsed_gr3)
    }
}

impl TryFrom<&Url> for Hgrid {
    type Error = HgridTryFromError;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let parsed_gr3 = gr3::parse_from_url(url)
            .map_err(|e| HgridTryFromError::TryFromUrlError(url.to_string(), e.to_string()))?;
        Hgrid::try_from(&parsed_gr3)
    }
}

impl TryFrom<&Gr3ParserOutput> for Hgrid {
    type Error = HgridTryFromError;

    fn try_from(parsed_gr3: &Gr3ParserOutput) -> Result<Self, Self::Error> {
        let nodes = NodesBuilder::default()
            .hash_map(parsed_gr3.nodes_values_reversed_sign())
            .crs(parsed_gr3.crs())
            .build()
            .map(Arc::new)?;
        let elements = ElementsBuilder::default()
            .nodes(nodes.clone())
            .hash_map(
                parsed_gr3
                    .elements()
                    .unwrap_or_else(|| LinkedHashMap::new()),
            )
            .build()?;
        let description = parsed_gr3.description();
        let is_open_boundary_present = parsed_gr3.open_boundaries().is_some()
            && parsed_gr3
                .open_boundaries()
                .as_ref()
                .map_or(false, |v| !v.is_empty());
        let is_land_boundary_present = parsed_gr3.land_boundaries().is_some()
            && parsed_gr3
                .land_boundaries()
                .as_ref()
                .map_or(false, |v| !v.is_empty());
        let is_interior_boundary_present = parsed_gr3.interior_boundaries().is_some()
            && parsed_gr3
                .interior_boundaries()
                .as_ref()
                .map_or(false, |v| !v.is_empty());
        let boundaries =
            if is_open_boundary_present || is_land_boundary_present || is_interior_boundary_present
            {
                let mut boundaries_builder = BoundariesBuilder::default();
                if is_open_boundary_present {
                    let mut open_boundary_builder = OpenBoundariesBuilder::default();
                    boundaries_builder.open(Some(
                        open_boundary_builder
                            .nodes_ids(parsed_gr3.open_boundaries().unwrap_or_else(Vec::new))
                            .nodes(nodes.clone())
                            .build()?,
                    ));
                }

                if is_land_boundary_present {
                    let mut land_boundary_builder = LandBoundariesBuilder::default();
                    boundaries_builder.land(Some(
                        land_boundary_builder
                            .nodes_ids(parsed_gr3.land_boundaries().unwrap_or_else(Vec::new))
                            .nodes(nodes.clone())
                            .build()?,
                    ));
                }

                if is_interior_boundary_present {
                    let mut interior_boundary_builder = InteriorBoundariesBuilder::default();
                    boundaries_builder.interior(Some(
                        interior_boundary_builder
                            .nodes_ids(parsed_gr3.interior_boundaries().unwrap_or_else(Vec::new))
                            .nodes(nodes.clone())
                            .build()?,
                    ));
                }

                Some(boundaries_builder.build()?)
            } else {
                None
            };

        Ok(Self {
            description,
            nodes,
            elements,
            boundaries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delaunator::{triangulate, Point};
    use log;
    use ndarray::Array1;
    use proj::Proj;
    use std::sync::Arc;
    use std::time::Instant;
    use tempfile::NamedTempFile;

    #[test]
    #[ignore]
    fn test_write_sample_nwatl_hgrid() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        let xmin = -98.00556;
        let ymin = 8.534422;
        let xmax = -60.040005;
        let ymax = 45.831431;
        let num_samples = 3000000;
        let num_x = (num_samples as f64 * ((xmax - xmin) / (ymax - ymin))).sqrt() as usize;
        let num_y = num_samples / num_x;
        let x_coords: Array1<f64> = Array1::linspace(xmin, xmax, num_x);
        let y_coords: Array1<f64> = Array1::linspace(ymin, ymax, num_y);
        let mut points: Vec<Point> = Vec::new();
        for x in x_coords.iter() {
            for y in y_coords.iter() {
                points.push(Point { x: *x, y: *y });
            }
        }

        log::info!("Begin making nodes hash map.");
        let start = Instant::now();
        let nodes_hash_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = points
            .iter()
            .enumerate()
            .map(|(index, point)| (index as u32, (vec![point.x, point.y], None)))
            .collect();
        log::debug!(
            "Begin making nodes hash map took {:?} seconds.",
            start.elapsed()
        );
        let transformer = Proj::new("epsg:4326").map(Arc::new).unwrap();
        log::info!("Begin making nodes struct.");
        let start = Instant::now();
        let nodes = NodesBuilder::default()
            .hash_map(nodes_hash_map)
            .crs(transformer)
            .build()
            .map(Arc::new)
            .unwrap();
        log::debug!(
            "Making nodes struct data took {:?} seconds.",
            start.elapsed()
        );
        log::info!("Begin Triangulation on mock data ({} nodes).", points.len());
        let start = Instant::now();
        let triangulation = triangulate(&points);
        log::debug!(
            "Triangulation of mock data took {:?} seconds.",
            start.elapsed()
        );
        log::info!("Begin making Elements hash_map.");
        let start = Instant::now();
        let elements_hash_map: LinkedHashMap<u32, Vec<u32>> = triangulation
            .triangles
            .chunks(3)
            .enumerate()
            .map(|(index, triangle)| {
                let triangle_u32: Vec<u32> = triangle.iter().map(|&vert| vert as u32).collect();
                (index as u32, triangle_u32)
            })
            .collect();
        log::debug!(
            "making elements hash map took {:?} seconds.",
            start.elapsed()
        );
        log::info!("Begin making Elements object.");
        let start = Instant::now();
        let elements = ElementsBuilder::default()
            .hash_map(elements_hash_map)
            .nodes(nodes.clone())
            .build()
            .unwrap();
        log::debug!(
            "Making elements object data took {:?} seconds.",
            start.elapsed()
        );
        log::info!("Begin making Hgrid");
        let hgrid = HgridBuilder::default()
            .nodes(nodes)
            .elements(elements)
            .boundaries(None)
            .description("mock hgrid grid NW ATL".to_owned())
            .build()
            .unwrap();
        log::debug!("Done making Hgrid!");
        log::info!("Begin writting Hgrid to {}", temp_path.display());
        let _result = hgrid.write(temp_path);
        log::debug!("Done writting hgrid to {}", temp_path.display());
    }
}
