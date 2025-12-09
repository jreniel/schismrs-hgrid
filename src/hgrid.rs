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

/// Depth sign convention for grid data.
///
/// SCHISM gr3 files use positive-down convention (positive values = below surface).
/// This is now the default internal storage convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepthConvention {
    /// Positive values = down (SCHISM file convention: 10.0 means 10m below surface)
    #[default]
    PositiveDown,
    /// Positive values = up / negative = down (elevation-style convention)
    PositiveUp,
}

impl DepthConvention {
    /// Returns the opposite convention
    pub fn flip(&self) -> Self {
        match self {
            DepthConvention::PositiveDown => DepthConvention::PositiveUp,
            DepthConvention::PositiveUp => DepthConvention::PositiveDown,
        }
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(setter(into))]
pub struct Hgrid {
    nodes: Arc<Nodes>,
    elements: Elements,
    boundaries: Option<Boundaries>,
    description: Option<String>,
    /// The depth sign convention used for values in this grid.
    /// Default is PositiveDown (SCHISM file convention).
    #[builder(default)]
    depth_convention: DepthConvention,
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

    /// Get depths as stored internally, respecting the current depth convention.
    ///
    /// By default (after loading from gr3), depths are stored in positive-down convention
    /// (SCHISM file convention): 10.0 means 10m below surface.
    ///
    /// Use `depth_convention()` to check the current convention.
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

    /// Get depths as positive-down values (matching gr3 file convention).
    ///
    /// This is the standard SCHISM/oceanographic convention where positive values
    /// indicate depth below the surface. A value of 10.0 means 10m below surface.
    ///
    /// If the internal convention is already PositiveDown, returns depths as-is.
    /// If the internal convention is PositiveUp, negates the values.
    pub fn depths_positive_down(&self) -> Array1<f64> {
        match self.depth_convention {
            DepthConvention::PositiveDown => self.depths(),
            DepthConvention::PositiveUp => self.depths().mapv(|d| -d),
        }
    }

    /// Get depths as positive-up values (elevation-style convention).
    ///
    /// In this convention, negative values indicate depth below the surface.
    /// A value of -10.0 means 10m below surface.
    ///
    /// If the internal convention is already PositiveUp, returns depths as-is.
    /// If the internal convention is PositiveDown, negates the values.
    pub fn depths_positive_up(&self) -> Array1<f64> {
        match self.depth_convention {
            DepthConvention::PositiveUp => self.depths(),
            DepthConvention::PositiveDown => self.depths().mapv(|d| -d),
        }
    }

    /// Returns the depth convention used for internal storage.
    pub fn depth_convention(&self) -> DepthConvention {
        self.depth_convention
    }

    /// Flip the depth values in place, changing the sign convention.
    ///
    /// This mutates the Hgrid by negating all depth values and updating
    /// the depth_convention field accordingly.
    ///
    /// After calling this method:
    /// - If convention was PositiveDown, it becomes PositiveUp
    /// - If convention was PositiveUp, it becomes PositiveDown
    /// - All depth values are negated
    pub fn flip_depths(&mut self) {
        // Get the current nodes hash map
        let old_nodes = self.nodes.hash_map();

        // Create new hash map with negated depth values
        let new_nodes: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = old_nodes
            .iter()
            .map(|(&node_id, (coord, value))| {
                let negated_value = value.as_ref().map(|v| v.iter().map(|&x| -x).collect());
                (node_id, (coord.clone(), negated_value))
            })
            .collect();

        // Rebuild the nodes with the new values
        let new_nodes_struct = NodesBuilder::default()
            .hash_map(new_nodes)
            .crs(self.nodes.crs())
            .build()
            .expect("Failed to rebuild nodes after flip_depths");

        // Update the Hgrid
        self.nodes = Arc::new(new_nodes_struct);
        self.depth_convention = self.depth_convention.flip();
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

        // gr3 files use positive-down convention
        // If we're in positive-down, write as-is; if positive-up, negate
        let output_nodes: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = match self
            .depth_convention
        {
            DepthConvention::PositiveDown => self.nodes.hash_map().clone(),
            DepthConvention::PositiveUp => self
                .nodes
                .hash_map()
                .iter()
                .map(|(&node_id, (coord, value))| {
                    let negated_value = value.as_ref().map(|v| v.iter().map(|&x| -x).collect());
                    (node_id, (coord.clone(), negated_value))
                })
                .collect(),
        };
        gr3_parser_output_builder.nodes(output_nodes);
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

    /// Check if the CRS is geographic (lon/lat based, e.g., EPSG:4326)
    ///
    /// Returns `true` if the CRS is geographic (uses angular units like degrees),
    /// `false` if it's projected (uses linear units like meters) or if no CRS is defined.
    ///
    /// Detection checks multiple sources:
    /// 1. The PROJ definition string for `+proj=longlat`
    /// 2. The ProjJSON representation for "GeographicCRS"
    pub fn is_geographic(&self) -> bool {
        self.crs()
            .map(|proj| {
                // First check proj_info definition
                if let Some(def) = proj.proj_info().definition.as_ref() {
                    let def_lower = def.to_lowercase();
                    if def_lower.contains("+proj=longlat") || def_lower.contains("proj=longlat") {
                        return true;
                    }
                    if def_lower.contains("geographic") || def_lower.contains("geodetic") {
                        return true;
                    }
                }

                // Check the full definition via def()
                if let Ok(full_def) = proj.def() {
                    let def_lower = full_def.to_lowercase();
                    if def_lower.contains("+proj=longlat") || def_lower.contains("proj=longlat") {
                        return true;
                    }
                }

                // Check ProjJSON for GeographicCRS - most reliable for EPSG codes
                if let Ok(json) = proj.to_projjson(None, None, None) {
                    if json.contains("GeographicCRS") {
                        return true;
                    }
                }

                false
            })
            .unwrap_or(false)
    }

    /// Get the CRS definition or description string if available
    ///
    /// Returns the proj definition string if available, otherwise falls back to
    /// the description (e.g., "WGS 84" for EPSG:4326).
    pub fn crs_definition(&self) -> Option<String> {
        self.crs()
            .and_then(|proj| {
                let info = proj.proj_info();
                // Try definition first (contains full PROJ string)
                if let Some(def) = info.definition.as_ref() {
                    if !def.is_empty() {
                        return Some(def.clone());
                    }
                }
                // Fall back to description (e.g., "WGS 84" for EPSG codes)
                info.description.clone()
            })
    }

    /// Compute the centroid of the mesh in lon/lat coordinates (EPSG:4326)
    ///
    /// If the mesh is already in geographic coordinates, returns the mean x/y directly.
    /// If the mesh is in a projected CRS, transforms the centroid to EPSG:4326.
    ///
    /// Returns `(longitude, latitude)` in degrees.
    pub fn centroid_lonlat(&self) -> Result<(f64, f64), HgridTryFromError> {
        let x = self.x();
        let y = self.y();

        let mean_x = x.mean().unwrap_or(0.0);
        let mean_y = y.mean().unwrap_or(0.0);

        if self.is_geographic() {
            // Already in lon/lat
            Ok((mean_x, mean_y))
        } else {
            // Need to get the CRS definition to create a transformer
            let src_def = self
                .crs_definition()
                .ok_or(HgridTryFromError::NoCrsDefined)?;

            // Create a transformer from source CRS to WGS84
            let transformer = Proj::new_known_crs(&src_def, "EPSG:4326", None)
                .map_err(|e| HgridTryFromError::ProjError(e.to_string()))?;

            let (lon, lat) = transformer
                .convert((mean_x, mean_y))
                .map_err(|e| HgridTryFromError::TransformError(e.to_string()))?;

            Ok((lon, lat))
        }
    }

    /// Transform all coordinates to a new CRS, returning a new Hgrid
    ///
    /// The source CRS must be defined on this Hgrid.
    /// `dst_crs` can be any valid PROJ string (e.g., "EPSG:4326", "EPSG:32618", etc.)
    pub fn transform_to(&self, dst_crs: &str) -> Result<Hgrid, HgridTryFromError> {
        let src_def = self
            .crs_definition()
            .ok_or(HgridTryFromError::NoCrsDefined)?;

        // Create a transformer from source CRS to destination CRS
        let transformer = Proj::new_known_crs(&src_def, dst_crs, None)
            .map_err(|e| HgridTryFromError::ProjError(e.to_string()))?;

        // Create a Proj object for the destination CRS (for storing in the result)
        let dst_proj = Proj::new(dst_crs)
            .map_err(|e| HgridTryFromError::ProjError(e.to_string()))?;

        // Transform all node coordinates
        let mut new_nodes: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = LinkedHashMap::new();

        for (node_id, (coords, values)) in self.nodes.hash_map().iter() {
            let (new_x, new_y) = transformer
                .convert((coords[0], coords[1]))
                .map_err(|e| HgridTryFromError::TransformError(e.to_string()))?;

            new_nodes.insert(*node_id, (vec![new_x, new_y], values.clone()));
        }

        // Build new Nodes with transformed coordinates
        let new_nodes_struct = NodesBuilder::default()
            .hash_map(new_nodes)
            .crs(Some(Arc::new(dst_proj)))
            .build()?;

        // Build new Hgrid with the same elements and boundaries but new nodes
        let new_nodes_arc = Arc::new(new_nodes_struct);

        // Rebuild elements with reference to new nodes
        let new_elements = ElementsBuilder::default()
            .nodes(new_nodes_arc.clone())
            .hash_map(self.elements.hash_map().clone())
            .build()?;

        // Rebuild boundaries if present
        let new_boundaries = if let Some(boundaries) = &self.boundaries {
            let type_map = boundaries.to_boundary_type_map();

            let mut boundaries_builder = BoundariesBuilder::default();

            if !type_map[&BoundaryType::Open].is_empty() {
                let open = OpenBoundariesBuilder::default()
                    .nodes_ids(type_map[&BoundaryType::Open].clone())
                    .nodes(new_nodes_arc.clone())
                    .build()?;
                boundaries_builder.open(Some(open));
            }

            if !type_map[&BoundaryType::Land].is_empty() {
                let land = LandBoundariesBuilder::default()
                    .nodes_ids(type_map[&BoundaryType::Land].clone())
                    .nodes(new_nodes_arc.clone())
                    .build()?;
                boundaries_builder.land(Some(land));
            }

            if !type_map[&BoundaryType::Interior].is_empty() {
                let interior = InteriorBoundariesBuilder::default()
                    .nodes_ids(type_map[&BoundaryType::Interior].clone())
                    .nodes(new_nodes_arc.clone())
                    .build()?;
                boundaries_builder.interior(Some(interior));
            }

            Some(boundaries_builder.build()?)
        } else {
            None
        };

        Ok(Hgrid {
            nodes: new_nodes_arc,
            elements: new_elements,
            boundaries: new_boundaries,
            description: self.description.clone(),
            depth_convention: self.depth_convention, // Preserve depth convention
        })
    }

    /// Transform coordinates to EPSG:4326 (WGS84 lon/lat)
    ///
    /// Convenience method for generating hgrid.ll.
    /// If the mesh is already in EPSG:4326, returns a clone.
    pub fn to_lonlat(&self) -> Result<Hgrid, HgridTryFromError> {
        if self.is_geographic() {
            // Check if it's specifically EPSG:4326 or another geographic CRS
            // For simplicity, if it's any geographic CRS we clone (they're all lon/lat)
            Ok(self.clone())
        } else {
            self.transform_to("EPSG:4326")
        }
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

    #[error("No CRS defined for hgrid - cannot perform coordinate transformation")]
    NoCrsDefined,

    #[error("Coordinate transformation failed: {0}")]
    TransformError(String),

    #[error("PROJ error: {0}")]
    ProjError(String),
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
        // Use nodes() directly - no reversal, keep positive-down convention from gr3 file
        let nodes = NodesBuilder::default()
            .hash_map(parsed_gr3.nodes())
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
            depth_convention: DepthConvention::PositiveDown, // gr3 uses positive-down
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

    #[test]
    fn test_epsg4326_is_geographic() {
        // Test that EPSG:4326 is correctly detected as geographic
        let proj = Proj::new("EPSG:4326").expect("Should parse EPSG:4326");

        // Check what def() returns
        if let Ok(def) = proj.def() {
            println!("EPSG:4326 def(): '{}'", def);
        }

        // Check proj_info
        let info = proj.proj_info();
        println!("EPSG:4326 proj_info.definition: {:?}", info.definition);

        // Check projjson
        if let Ok(json) = proj.to_projjson(None, None, None) {
            println!("EPSG:4326 projjson (first 500 chars): {}", &json[..json.len().min(500)]);
            // ProjJSON should contain "GeographicCRS" for geographic systems
            assert!(
                json.contains("GeographicCRS") || json.contains("geographic"),
                "EPSG:4326 projjson should indicate geographic CRS"
            );
        }
    }

    #[test]
    fn test_utm_is_not_geographic() {
        // Test that UTM is NOT detected as geographic
        let proj = Proj::new("EPSG:32618").expect("Should parse EPSG:32618 (UTM 18N)");

        if let Ok(def) = proj.def() {
            println!("EPSG:32618 def(): {}", def);
            assert!(
                !def.to_lowercase().contains("+proj=longlat"),
                "UTM should NOT contain +proj=longlat, got: {}",
                def
            );
        }
    }

    #[test]
    fn test_depth_convention_methods() {
        // Create a simple test mesh with known depths
        // Using positive-down convention (SCHISM file format)
        let mut nodes_hash_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> =
            LinkedHashMap::new();

        // Add nodes with depths in positive-down convention
        // 10.0 means 10m below surface
        nodes_hash_map.insert(0, (vec![0.0, 0.0], Some(vec![10.0])));
        nodes_hash_map.insert(1, (vec![1.0, 0.0], Some(vec![20.0])));
        nodes_hash_map.insert(2, (vec![0.5, 1.0], Some(vec![5.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes_hash_map)
            .crs(None::<Arc<Proj>>)
            .build()
            .map(Arc::new)
            .unwrap();

        let mut elements_hash_map: LinkedHashMap<u32, Vec<u32>> = LinkedHashMap::new();
        elements_hash_map.insert(0, vec![0, 1, 2]);

        let elements = ElementsBuilder::default()
            .nodes(nodes.clone())
            .hash_map(elements_hash_map)
            .build()
            .unwrap();

        let hgrid = HgridBuilder::default()
            .nodes(nodes)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap();

        // Test depth_convention - default is PositiveDown
        assert_eq!(hgrid.depth_convention(), DepthConvention::PositiveDown);

        // Test depths() returns internal values (positive-down)
        let depths = hgrid.depths();
        assert_eq!(depths.len(), 3);
        // Values should be positive (positive-down convention)
        assert!(depths.iter().all(|&d| d >= 0.0));

        // Test depths_positive_down() returns positive values
        let depths_pd = hgrid.depths_positive_down();
        assert_eq!(depths_pd.len(), 3);
        // Values should be positive (same as internal)
        assert!(depths_pd.iter().all(|&d| d >= 0.0));

        // Verify the values are correct
        assert!((depths_pd[0] - 10.0).abs() < 1e-9);
        assert!((depths_pd[1] - 20.0).abs() < 1e-9);
        assert!((depths_pd[2] - 5.0).abs() < 1e-9);

        // Test depths_positive_up() returns negative values
        let depths_pu = hgrid.depths_positive_up();
        assert!(depths_pu.iter().all(|&d| d <= 0.0));
        assert!((depths_pu[0] - (-10.0)).abs() < 1e-9);
        assert!((depths_pu[1] - (-20.0)).abs() < 1e-9);
        assert!((depths_pu[2] - (-5.0)).abs() < 1e-9);
    }

    #[test]
    fn test_flip_depths() {
        // Create a test mesh with positive-down convention (need 3 nodes for a triangle)
        let mut nodes_hash_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> =
            LinkedHashMap::new();

        nodes_hash_map.insert(0, (vec![0.0, 0.0], Some(vec![10.0])));
        nodes_hash_map.insert(1, (vec![1.0, 0.0], Some(vec![20.0])));
        nodes_hash_map.insert(2, (vec![0.5, 1.0], Some(vec![15.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes_hash_map)
            .crs(None::<Arc<Proj>>)
            .build()
            .map(Arc::new)
            .unwrap();

        let mut elements_hash_map: LinkedHashMap<u32, Vec<u32>> = LinkedHashMap::new();
        elements_hash_map.insert(0, vec![0, 1, 2]); // Triangle element

        let elements = ElementsBuilder::default()
            .nodes(nodes.clone())
            .hash_map(elements_hash_map)
            .build()
            .unwrap();

        let mut hgrid = HgridBuilder::default()
            .nodes(nodes)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap();

        // Initially positive-down
        assert_eq!(hgrid.depth_convention(), DepthConvention::PositiveDown);
        let depths_before = hgrid.depths();
        assert!((depths_before[0] - 10.0).abs() < 1e-9);
        assert!((depths_before[1] - 20.0).abs() < 1e-9);
        assert!((depths_before[2] - 15.0).abs() < 1e-9);

        // Flip to positive-up
        hgrid.flip_depths();
        assert_eq!(hgrid.depth_convention(), DepthConvention::PositiveUp);
        let depths_after = hgrid.depths();
        assert!((depths_after[0] - (-10.0)).abs() < 1e-9);
        assert!((depths_after[1] - (-20.0)).abs() < 1e-9);
        assert!((depths_after[2] - (-15.0)).abs() < 1e-9);

        // depths_positive_down should still give positive values
        let depths_pd = hgrid.depths_positive_down();
        assert!((depths_pd[0] - 10.0).abs() < 1e-9);
        assert!((depths_pd[1] - 20.0).abs() < 1e-9);
        assert!((depths_pd[2] - 15.0).abs() < 1e-9);

        // Flip back to positive-down
        hgrid.flip_depths();
        assert_eq!(hgrid.depth_convention(), DepthConvention::PositiveDown);
        let depths_final = hgrid.depths();
        assert!((depths_final[0] - 10.0).abs() < 1e-9);
        assert!((depths_final[1] - 20.0).abs() < 1e-9);
        assert!((depths_final[2] - 15.0).abs() < 1e-9);
    }

    #[test]
    #[ignore] // Requires the dev hgrid file to exist
    fn test_load_dev_hgrid_epsg4326() {
        // Test loading the actual dev hgrid file
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("schismrs/dev/hgrid.gr3");

        if !path.exists() {
            println!("Skipping test - dev hgrid not found at {:?}", path);
            return;
        }

        let hgrid = Hgrid::try_from(&path).expect("Should load hgrid");

        // Check CRS was parsed
        println!("CRS: {:?}", hgrid.crs());
        println!("is_geographic: {}", hgrid.is_geographic());
        println!("CRS definition: {:?}", hgrid.crs_definition());
    }
}
