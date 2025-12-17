// src/hgrid/src/boundary_polygon.rs
//
// Mesh boundary polygon extraction for point-in-mesh containment testing.
// Ported from pyschism's mesh.base module.

use crate::Hgrid;
use ndarray::Array2;
use std::collections::HashMap;

/// Cached boundary polygon data for efficient containment testing
#[derive(Debug, Clone)]
pub struct BoundaryPolygon {
    /// Boundary vertices as Nx2 array (x, y coordinates)
    pub nodes: Array2<f64>,
    /// Edge connectivity as Ex2 array (indices into nodes)
    pub edges: Array2<usize>,
}

impl Hgrid {
    /// Extract the mesh boundary polygon for containment testing.
    ///
    /// Returns boundary nodes and edges suitable for use with inpoly2.
    /// Handles multiple rings (exterior boundary + island holes).
    ///
    /// Algorithm:
    /// 1. Find boundary edges (edges appearing in only one element)
    /// 2. Order edges into closed rings
    /// 3. Return nodes and edges for inpoly
    pub fn boundary_polygon(&self) -> BoundaryPolygon {
        // Step 1: Find boundary edges
        let boundary_edges = self.find_boundary_edges();

        if boundary_edges.is_empty() {
            return BoundaryPolygon {
                nodes: Array2::zeros((0, 2)),
                edges: Array2::zeros((0, 2)),
            };
        }

        // Step 2: Order edges into rings
        let rings = edges_to_rings(boundary_edges);

        // Step 3: Build node and edge arrays for inpoly
        self.build_inpoly_arrays(&rings)
    }

    /// Check if a point is inside the mesh domain.
    ///
    /// Uses boundary polygon extraction + inpoly for fast containment testing.
    /// Handles islands (interior holes) correctly.
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        let boundary = self.boundary_polygon();

        if boundary.nodes.is_empty() {
            return false;
        }

        // Create a single point as Nx2 array
        let vert = Array2::from_shape_vec((1, 2), vec![x, y]).unwrap();

        // Call inpoly2
        let (inside, _on_edge) = inpoly::inpoly2(&vert, &boundary.nodes, Some(&boundary.edges), None);

        inside[0]
    }

    /// Check if multiple points are inside the mesh domain.
    ///
    /// More efficient than calling contains_point repeatedly.
    pub fn contains_points(&self, points: &[(f64, f64)]) -> Vec<bool> {
        let boundary = self.boundary_polygon();

        if boundary.nodes.is_empty() || points.is_empty() {
            return vec![false; points.len()];
        }

        // Create points as Nx2 array
        let mut vert_data = Vec::with_capacity(points.len() * 2);
        for (x, y) in points {
            vert_data.push(*x);
            vert_data.push(*y);
        }
        let vert = Array2::from_shape_vec((points.len(), 2), vert_data).unwrap();

        // Call inpoly2
        let (inside, _on_edge) = inpoly::inpoly2(&vert, &boundary.nodes, Some(&boundary.edges), None);

        inside.to_vec()
    }

    /// Find boundary edges (edges that appear in only one element).
    ///
    /// An edge is on the boundary if it belongs to exactly one element.
    /// Edges are stored as (smaller_node_id, larger_node_id) for deduplication.
    fn find_boundary_edges(&self) -> Vec<(u32, u32)> {
        let mut edge_count: HashMap<(u32, u32), usize> = HashMap::new();

        for node_ids in self.elements().hash_map().values() {
            let n = node_ids.len();
            for i in 0..n {
                let a = node_ids[i];
                let b = node_ids[(i + 1) % n];
                // Normalize edge direction for counting
                let edge = if a < b { (a, b) } else { (b, a) };
                *edge_count.entry(edge).or_insert(0) += 1;
            }
        }

        // Return edges that appear exactly once
        edge_count
            .into_iter()
            .filter(|(_, count)| *count == 1)
            .map(|(edge, _)| edge)
            .collect()
    }

    /// Build inpoly-compatible node and edge arrays from ordered rings.
    fn build_inpoly_arrays(&self, rings: &[Vec<(u32, u32)>]) -> BoundaryPolygon {
        let xs = self.x();
        let ys = self.y();

        // Collect all unique nodes from all rings
        let mut all_nodes: Vec<u32> = Vec::new();
        let mut node_to_idx: HashMap<u32, usize> = HashMap::new();

        for ring in rings {
            for (a, _b) in ring {
                if !node_to_idx.contains_key(a) {
                    node_to_idx.insert(*a, all_nodes.len());
                    all_nodes.push(*a);
                }
            }
        }

        // Build nodes array (Nx2)
        let mut nodes_data = Vec::with_capacity(all_nodes.len() * 2);
        for node_id in &all_nodes {
            // Node IDs are 1-indexed
            let idx = (*node_id as usize) - 1;
            nodes_data.push(xs[idx]);
            nodes_data.push(ys[idx]);
        }
        let nodes = Array2::from_shape_vec((all_nodes.len(), 2), nodes_data).unwrap();

        // Build edges array (Ex2)
        let total_edges: usize = rings.iter().map(|r| r.len()).sum();
        let mut edges_data = Vec::with_capacity(total_edges * 2);

        for ring in rings {
            for (a, b) in ring {
                edges_data.push(node_to_idx[a]);
                edges_data.push(node_to_idx[b]);
            }
        }
        let edges = Array2::from_shape_vec((total_edges, 2), edges_data).unwrap();

        BoundaryPolygon { nodes, edges }
    }
}

/// Order boundary edges into closed rings.
///
/// Takes a list of unordered edges and chains them into closed loops.
/// Each ring is a vector of edges (node_a, node_b) in order.
fn edges_to_rings(mut edges: Vec<(u32, u32)>) -> Vec<Vec<(u32, u32)>> {
    if edges.is_empty() {
        return Vec::new();
    }

    let mut rings: Vec<Vec<(u32, u32)>> = Vec::new();

    while !edges.is_empty() {
        // Start a new ring with the last edge
        let mut ring = vec![edges.pop().unwrap()];

        // Build adjacency for quick lookup
        // We need to find edges that connect to our current ring endpoints
        loop {
            let ring_start = ring.first().unwrap().0;
            let ring_end = ring.last().unwrap().1;

            // Check if ring is closed
            if ring_start == ring_end && ring.len() > 1 {
                break;
            }

            // Find an edge that connects to ring_end
            let mut found = false;
            for i in 0..edges.len() {
                let (a, b) = edges[i];
                if a == ring_end {
                    ring.push(edges.remove(i));
                    found = true;
                    break;
                } else if b == ring_end {
                    // Reverse the edge
                    ring.push((b, a));
                    edges.remove(i);
                    found = true;
                    break;
                }
            }

            if !found {
                // Try to extend from the start
                for i in 0..edges.len() {
                    let (a, b) = edges[i];
                    if b == ring_start {
                        ring.insert(0, edges.remove(i));
                        found = true;
                        break;
                    } else if a == ring_start {
                        ring.insert(0, (b, a));
                        edges.remove(i);
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                // Can't extend further, ring is complete (or there's an error)
                break;
            }
        }

        rings.push(ring);
    }

    rings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HgridBuilder;
    use crate::nodes::NodesBuilder;
    use crate::elements::ElementsBuilder;
    use linked_hash_map::LinkedHashMap;
    use std::sync::Arc;

    fn make_simple_triangle_mesh() -> Hgrid {
        // Simple mesh: single triangle
        //
        //     3
        //    / \
        //   /   \
        //  1-----2
        //
        let mut nodes_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = LinkedHashMap::new();
        nodes_map.insert(1u32, (vec![0.0, 0.0], Some(vec![0.0])));
        nodes_map.insert(2, (vec![1.0, 0.0], Some(vec![0.0])));
        nodes_map.insert(3, (vec![0.5, 1.0], Some(vec![0.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes_map)
            .build()
            .unwrap();

        let mut elements_map = LinkedHashMap::new();
        elements_map.insert(1u32, vec![1u32, 2, 3]);

        let nodes_arc = Arc::new(nodes);
        let elements = ElementsBuilder::default()
            .hash_map(elements_map)
            .nodes(nodes_arc.clone())
            .build()
            .unwrap();

        HgridBuilder::default()
            .nodes(nodes_arc)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap()
    }

    fn make_two_triangle_mesh() -> Hgrid {
        // Two triangles forming a square:
        //
        //  3-----4
        //  |\    |
        //  | \   |
        //  |  \  |
        //  |   \ |
        //  1-----2
        //
        let mut nodes_map: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> = LinkedHashMap::new();
        nodes_map.insert(1u32, (vec![0.0, 0.0], Some(vec![0.0])));
        nodes_map.insert(2, (vec![1.0, 0.0], Some(vec![0.0])));
        nodes_map.insert(3, (vec![0.0, 1.0], Some(vec![0.0])));
        nodes_map.insert(4, (vec![1.0, 1.0], Some(vec![0.0])));

        let nodes = NodesBuilder::default()
            .hash_map(nodes_map)
            .build()
            .unwrap();

        let mut elements_map = LinkedHashMap::new();
        elements_map.insert(1u32, vec![1u32, 2, 3]);
        elements_map.insert(2, vec![2u32, 4, 3]);

        let nodes_arc = Arc::new(nodes);
        let elements = ElementsBuilder::default()
            .hash_map(elements_map)
            .nodes(nodes_arc.clone())
            .build()
            .unwrap();

        HgridBuilder::default()
            .nodes(nodes_arc)
            .elements(elements)
            .boundaries(None)
            .description(None::<String>)
            .build()
            .unwrap()
    }

    #[test]
    fn test_find_boundary_edges_single_triangle() {
        let hgrid = make_simple_triangle_mesh();
        let boundary_edges = hgrid.find_boundary_edges();

        // Single triangle: all 3 edges are on boundary
        assert_eq!(boundary_edges.len(), 3);
    }

    #[test]
    fn test_find_boundary_edges_two_triangles() {
        let hgrid = make_two_triangle_mesh();
        let boundary_edges = hgrid.find_boundary_edges();

        // Two triangles share edge (2,3), so 4 boundary edges
        assert_eq!(boundary_edges.len(), 4);

        // The shared edge (2,3) should NOT be in boundary
        let normalized: Vec<_> = boundary_edges
            .iter()
            .map(|&(a, b)| if a < b { (a, b) } else { (b, a) })
            .collect();
        assert!(!normalized.contains(&(2, 3)));
    }

    #[test]
    fn test_edges_to_rings_single_triangle() {
        let edges = vec![(1, 2), (2, 3), (3, 1)];
        let rings = edges_to_rings(edges);

        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].len(), 3);
    }

    #[test]
    fn test_contains_point_single_triangle() {
        let hgrid = make_simple_triangle_mesh();

        // Point inside triangle
        assert!(hgrid.contains_point(0.5, 0.3));

        // Point outside triangle
        assert!(!hgrid.contains_point(2.0, 0.0));
        assert!(!hgrid.contains_point(0.5, 2.0));
        assert!(!hgrid.contains_point(-1.0, 0.5));
    }

    #[test]
    fn test_contains_point_two_triangles() {
        let hgrid = make_two_triangle_mesh();

        // Points inside the square
        assert!(hgrid.contains_point(0.5, 0.5));
        assert!(hgrid.contains_point(0.1, 0.1));
        assert!(hgrid.contains_point(0.9, 0.9));

        // Points outside
        assert!(!hgrid.contains_point(-0.1, 0.5));
        assert!(!hgrid.contains_point(1.1, 0.5));
        assert!(!hgrid.contains_point(0.5, -0.1));
        assert!(!hgrid.contains_point(0.5, 1.1));
    }

    #[test]
    fn test_contains_points_batch() {
        let hgrid = make_two_triangle_mesh();

        let points = vec![
            (0.5, 0.5),   // inside
            (2.0, 0.5),   // outside
            (0.1, 0.1),   // inside
            (-1.0, -1.0), // outside
        ];

        let results = hgrid.contains_points(&points);

        assert_eq!(results, vec![true, false, true, false]);
    }
}
