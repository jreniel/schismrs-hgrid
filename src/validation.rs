//! Mesh validation for Hgrid structures.
//!
//! Provides comprehensive validation checks for unstructured meshes:
//! - Structural checks (node references, boundary references)
//! - Geometric checks (element areas, orientation, concavity)

use crate::Hgrid;
use std::collections::HashSet;

/// Result of mesh validation containing all detected issues.
///
/// Use `is_ok()` for a quick pass/fail check, or inspect individual
/// fields for detailed information about specific issues.
#[derive(Debug, Clone, Default)]
pub struct MeshValidation {
    // Structural issues
    /// Element IDs that reference non-existent node IDs
    pub invalid_element_node_refs: Vec<(u32, Vec<u32>)>,
    /// Open boundary segments with node IDs not in the mesh
    pub invalid_open_boundary_refs: Vec<u32>,
    /// Land boundary segments with node IDs not in the mesh
    pub invalid_land_boundary_refs: Vec<u32>,
    /// Interior boundary segments with node IDs not in the mesh
    pub invalid_interior_boundary_refs: Vec<u32>,

    // Geometric issues
    /// Elements with negative signed area (clockwise winding, should be counter-clockwise)
    pub negative_area_elements: Vec<u32>,
    /// Elements with zero or near-zero area (degenerate)
    pub zero_area_elements: Vec<u32>,
    /// Quad elements that are concave
    pub concave_quads: Vec<u32>,
    /// Pairs of adjacent elements with inconsistent edge orientation
    pub orientation_conflicts: Vec<(u32, u32)>,
}

impl MeshValidation {
    /// Returns true if all validation checks passed.
    pub fn is_ok(&self) -> bool {
        self.invalid_element_node_refs.is_empty()
            && self.invalid_open_boundary_refs.is_empty()
            && self.invalid_land_boundary_refs.is_empty()
            && self.invalid_interior_boundary_refs.is_empty()
            && self.negative_area_elements.is_empty()
            && self.zero_area_elements.is_empty()
            && self.concave_quads.is_empty()
            && self.orientation_conflicts.is_empty()
    }

    /// Returns true if structural checks passed (ignoring geometric issues).
    pub fn is_structurally_valid(&self) -> bool {
        self.invalid_element_node_refs.is_empty()
            && self.invalid_open_boundary_refs.is_empty()
            && self.invalid_land_boundary_refs.is_empty()
            && self.invalid_interior_boundary_refs.is_empty()
    }

    /// Returns true if geometric checks passed (ignoring structural issues).
    pub fn is_geometrically_valid(&self) -> bool {
        self.negative_area_elements.is_empty()
            && self.zero_area_elements.is_empty()
            && self.concave_quads.is_empty()
            && self.orientation_conflicts.is_empty()
    }

    /// Total count of all issues found.
    pub fn issue_count(&self) -> usize {
        self.invalid_element_node_refs.len()
            + self.invalid_open_boundary_refs.len()
            + self.invalid_land_boundary_refs.len()
            + self.invalid_interior_boundary_refs.len()
            + self.negative_area_elements.len()
            + self.zero_area_elements.len()
            + self.concave_quads.len()
            + self.orientation_conflicts.len()
    }
}

impl std::fmt::Display for MeshValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ok() {
            write!(f, "Mesh validation: OK")
        } else {
            writeln!(f, "Mesh validation: {} issues found", self.issue_count())?;
            if !self.invalid_element_node_refs.is_empty() {
                writeln!(f, "  - {} elements with invalid node refs", self.invalid_element_node_refs.len())?;
            }
            if !self.invalid_open_boundary_refs.is_empty() {
                writeln!(f, "  - {} open boundaries with invalid node refs", self.invalid_open_boundary_refs.len())?;
            }
            if !self.invalid_land_boundary_refs.is_empty() {
                writeln!(f, "  - {} land boundaries with invalid node refs", self.invalid_land_boundary_refs.len())?;
            }
            if !self.invalid_interior_boundary_refs.is_empty() {
                writeln!(f, "  - {} interior boundaries with invalid node refs", self.invalid_interior_boundary_refs.len())?;
            }
            if !self.negative_area_elements.is_empty() {
                writeln!(f, "  - {} elements with negative area", self.negative_area_elements.len())?;
            }
            if !self.zero_area_elements.is_empty() {
                writeln!(f, "  - {} degenerate (zero-area) elements", self.zero_area_elements.len())?;
            }
            if !self.concave_quads.is_empty() {
                writeln!(f, "  - {} concave quad elements", self.concave_quads.len())?;
            }
            if !self.orientation_conflicts.is_empty() {
                writeln!(f, "  - {} orientation conflicts", self.orientation_conflicts.len())?;
            }
            Ok(())
        }
    }
}

/// Tolerance for zero-area detection
const AREA_TOL: f64 = 1e-10;

impl Hgrid {
    /// Perform comprehensive mesh validation.
    ///
    /// This checks both structural validity (node references) and
    /// geometric validity (element areas, orientation, concavity).
    ///
    /// # Example
    /// ```ignore
    /// let hgrid = Hgrid::try_from(&path)?;
    /// let validation = hgrid.check_validity();
    /// if !validation.is_ok() {
    ///     eprintln!("{}", validation);
    /// }
    /// ```
    pub fn check_validity(&self) -> MeshValidation {
        let mut result = MeshValidation::default();

        // Build node ID set once
        let node_ids: HashSet<u32> = self.nodes().hash_map().keys().copied().collect();

        // Check element node references
        self.check_element_refs(&node_ids, &mut result);

        // Check boundary node references
        self.check_boundary_refs(&node_ids, &mut result);

        // Check element geometry
        self.check_element_geometry(&mut result);

        // Check orientation consistency
        self.check_orientation_consistency(&mut result);

        result
    }

    /// Check that all element node IDs reference existing nodes.
    fn check_element_refs(&self, node_ids: &HashSet<u32>, result: &mut MeshValidation) {
        for (elem_id, elem_nodes) in self.elements().hash_map().iter() {
            let invalid: Vec<u32> = elem_nodes
                .iter()
                .filter(|n| !node_ids.contains(n))
                .copied()
                .collect();
            if !invalid.is_empty() {
                result.invalid_element_node_refs.push((*elem_id, invalid));
            }
        }
    }

    /// Check that all boundary node IDs reference existing nodes.
    fn check_boundary_refs(&self, node_ids: &HashSet<u32>, result: &mut MeshValidation) {
        if let Some(boundaries) = self.boundaries() {
            // Check open boundaries
            if let Some(open) = boundaries.open() {
                for (idx, segment) in open.nodes_ids().iter().enumerate() {
                    if segment.iter().any(|n| !node_ids.contains(n)) {
                        result.invalid_open_boundary_refs.push(idx as u32);
                    }
                }
            }

            // Check land boundaries
            let type_map = boundaries.to_boundary_type_map();
            if let Some(land_ids) = type_map.get(&crate::boundaries::BoundaryType::Land) {
                for (idx, segment) in land_ids.iter().enumerate() {
                    if segment.iter().any(|n| !node_ids.contains(n)) {
                        result.invalid_land_boundary_refs.push(idx as u32);
                    }
                }
            }

            // Check interior boundaries
            if let Some(interior_ids) = type_map.get(&crate::boundaries::BoundaryType::Interior) {
                for (idx, segment) in interior_ids.iter().enumerate() {
                    if segment.iter().any(|n| !node_ids.contains(n)) {
                        result.invalid_interior_boundary_refs.push(idx as u32);
                    }
                }
            }
        }
    }

    /// Check element geometry: areas and concavity.
    fn check_element_geometry(&self, result: &mut MeshValidation) {
        let nodes_map = self.nodes().hash_map();

        for (elem_id, elem_nodes) in self.elements().hash_map().iter() {
            // Get coordinates
            let coords: Vec<(f64, f64)> = elem_nodes
                .iter()
                .filter_map(|n| nodes_map.get(n))
                .map(|(coord, _)| (coord[0], coord[1]))
                .collect();

            if coords.len() != elem_nodes.len() {
                // Missing nodes - already caught by structural check
                continue;
            }

            if coords.len() == 3 {
                // Triangle
                let area = signed_triangle_area(coords[0], coords[1], coords[2]);
                if area < -AREA_TOL {
                    result.negative_area_elements.push(*elem_id);
                } else if area.abs() <= AREA_TOL {
                    result.zero_area_elements.push(*elem_id);
                }
            } else if coords.len() == 4 {
                // Quad - check both triangulations
                let area1 = signed_triangle_area(coords[0], coords[1], coords[2]);
                let area2 = signed_triangle_area(coords[0], coords[2], coords[3]);
                let total_area = area1 + area2;

                if total_area < -AREA_TOL {
                    result.negative_area_elements.push(*elem_id);
                } else if total_area.abs() <= AREA_TOL {
                    result.zero_area_elements.push(*elem_id);
                }

                // Check for concavity - all 4 sub-triangles should have same sign
                let area3 = signed_triangle_area(coords[0], coords[1], coords[3]);
                let area4 = signed_triangle_area(coords[1], coords[2], coords[3]);
                let areas = [area1, area2, area3, area4];
                let min_area = areas.iter().cloned().fold(f64::INFINITY, f64::min);

                if min_area <= -AREA_TOL {
                    result.concave_quads.push(*elem_id);
                }
            }
        }
    }

    /// Check for orientation conflicts between adjacent elements.
    fn check_orientation_consistency(&self, result: &mut MeshValidation) {
        use std::collections::HashMap;

        // Build edge -> (element_id, is_forward) map
        // An edge (a,b) stored as-is is "forward", stored as (b,a) is "backward"
        // Adjacent elements should have opposite edge directions
        let mut edge_map: HashMap<(u32, u32), (u32, bool)> = HashMap::new();

        for (elem_id, elem_nodes) in self.elements().hash_map().iter() {
            let n = elem_nodes.len();
            for i in 0..n {
                let a = elem_nodes[i];
                let b = elem_nodes[(i + 1) % n];

                // Canonical edge representation (smaller id first)
                let canonical = if a < b { (a, b) } else { (b, a) };
                let is_forward = a < b;

                if let Some(&(other_elem, other_forward)) = edge_map.get(&canonical) {
                    // Edge already seen - check if orientations are opposite
                    if is_forward == other_forward {
                        // Same direction = orientation conflict
                        result.orientation_conflicts.push((other_elem, *elem_id));
                    }
                } else {
                    edge_map.insert(canonical, (*elem_id, is_forward));
                }
            }
        }
    }
}

/// Compute signed area of a triangle using the shoelace formula.
/// Positive = counter-clockwise, negative = clockwise.
#[inline]
fn signed_triangle_area(v1: (f64, f64), v2: (f64, f64), v3: (f64, f64)) -> f64 {
    0.5 * ((v1.0 - v3.0) * (v2.1 - v3.1) - (v2.0 - v3.0) * (v1.1 - v3.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::ElementsBuilder;
    use crate::hgrid::HgridBuilder;
    use crate::nodes::NodesBuilder;
    use linked_hash_map::LinkedHashMap;
    use std::sync::Arc;

    fn make_simple_triangle_mesh() -> Hgrid {
        // Counter-clockwise triangle (positive area)
        let mut nodes = LinkedHashMap::new();
        nodes.insert(1, (vec![0.0, 0.0], Some(vec![-10.0])));
        nodes.insert(2, (vec![1.0, 0.0], Some(vec![-10.0])));
        nodes.insert(3, (vec![0.5, 1.0], Some(vec![-10.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes)
            .crs(None::<std::sync::Arc<proj::Proj>>)
            .build()
            .map(Arc::new)
            .unwrap();

        let mut elements = LinkedHashMap::new();
        elements.insert(1, vec![1, 2, 3]);

        let elements = ElementsBuilder::default()
            .hash_map(elements)
            .nodes(nodes.clone())
            .build()
            .unwrap();

        HgridBuilder::default()
            .nodes(nodes)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap()
    }

    #[test]
    fn test_valid_mesh() {
        let hgrid = make_simple_triangle_mesh();
        let validation = hgrid.check_validity();
        assert!(validation.is_ok());
        assert!(validation.is_structurally_valid());
        assert!(validation.is_geometrically_valid());
    }

    #[test]
    fn test_negative_area_detection() {
        // Clockwise triangle (negative area)
        let mut nodes = LinkedHashMap::new();
        nodes.insert(1, (vec![0.0, 0.0], Some(vec![-10.0])));
        nodes.insert(2, (vec![0.5, 1.0], Some(vec![-10.0]))); // Swapped 2 and 3
        nodes.insert(3, (vec![1.0, 0.0], Some(vec![-10.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes)
            .crs(None::<std::sync::Arc<proj::Proj>>)
            .build()
            .map(Arc::new)
            .unwrap();

        let mut elements = LinkedHashMap::new();
        elements.insert(1, vec![1, 2, 3]);

        let elements = ElementsBuilder::default()
            .hash_map(elements)
            .nodes(nodes.clone())
            .build()
            .unwrap();

        let hgrid = HgridBuilder::default()
            .nodes(nodes)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap();

        let validation = hgrid.check_validity();
        assert!(!validation.is_ok());
        assert!(validation.is_structurally_valid());
        assert!(!validation.is_geometrically_valid());
        assert_eq!(validation.negative_area_elements.len(), 1);
        assert_eq!(validation.negative_area_elements[0], 1);
    }

    #[test]
    fn test_display() {
        let hgrid = make_simple_triangle_mesh();
        let validation = hgrid.check_validity();
        let display = format!("{}", validation);
        assert!(display.contains("OK"));
    }
}
