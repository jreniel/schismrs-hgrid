// schismrs-hgrid/src/hash.rs

//! Hash implementation for Hgrid structs
//!
//! Provides deterministic hashing for Hgrid objects to enable
//! efficient change detection and caching.

use crate::Hgrid;
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl Hash for Hgrid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the description
        self.description().hash(state);

        // Hash nodes in insertion order (LinkedHashMap preserves order)
        // No sorting needed - iteration order is deterministic
        for (node_id, (coords, values)) in self.nodes().hash_map().iter() {
            node_id.hash(state);
            // f64 doesn't impl Hash, so hash the bytes
            for coord in coords {
                state.write(&coord.to_le_bytes());
            }
            if let Some(vals) = values {
                for val in vals {
                    state.write(&val.to_le_bytes());
                }
            }
        }

        // Hash elements in insertion order (LinkedHashMap preserves order)
        for (element_id, node_list) in self.elements().hash_map().iter() {
            element_id.hash(state);
            node_list.hash(state);
        }

        // Hash boundaries if present
        if let Some(boundaries) = self.boundaries() {
            boundaries.hash(state);
        }

        // Hash CRS if present (crs is now stored as a string)
        if let Some(crs) = self.crs() {
            crs.hash(state);
        }
    }
}

impl Hgrid {
    /// Calculate a deterministic SHA256 hash of this Hgrid
    ///
    /// This can be used for change detection and caching.
    /// The hash is deterministic - the same Hgrid will always
    /// produce the same hash regardless of when it's computed.
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash description
        hasher.update(self.description().unwrap_or(&String::new()).as_bytes());

        // Hash node data directly in insertion order (LinkedHashMap is deterministic)
        for (node_id, (coords, values)) in self.nodes().hash_map().iter() {
            hasher.update(&node_id.to_le_bytes());
            for coord in coords {
                hasher.update(&coord.to_le_bytes());
            }
            if let Some(vals) = values {
                for val in vals {
                    hasher.update(&val.to_le_bytes());
                }
            }
        }

        // Hash element data directly in insertion order
        for (element_id, node_list) in self.elements().hash_map().iter() {
            hasher.update(&element_id.to_le_bytes());
            for node_id in node_list {
                hasher.update(&node_id.to_le_bytes());
            }
        }

        // Hash boundaries if present
        if let Some(boundaries) = self.boundaries() {
            if let Some(open) = boundaries.open() {
                for boundary in open.nodes_ids() {
                    for node_id in boundary {
                        hasher.update(&node_id.to_le_bytes());
                    }
                }
            }
        }

        // Hash CRS if present (crs is now stored as a string)
        if let Some(crs) = self.crs() {
            hasher.update(crs.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Quick hash for change detection
    ///
    /// This is faster than calculate_hash() but may have more collisions.
    /// Use this for quick comparisons, calculate_hash() for storage.
    pub fn quick_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

// We need to implement Hash for the component types as well
use crate::boundaries::{Boundaries, OpenBoundaries};

impl Hash for Boundaries {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.open().is_some().hash(state);
        if let Some(open) = self.open() {
            open.hash(state);
        }

        // Note: We'd need to add getters for land and interior boundaries
        // For now, just hash whether they exist
        // TODO: Add proper accessors to Boundaries
    }
}

impl Hash for OpenBoundaries {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.nodes_ids().hash(state);
    }
}

// TODO: Add Hash implementations for LandBoundaries and InteriorBoundaries
// when we add proper accessors to the Boundaries struct

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_hgrid_hash_deterministic() {
        // Create a test hgrid file
        let temp_dir = tempdir().unwrap();
        let hgrid_path = temp_dir.path().join("test.gr3");

        let mut file = std::fs::File::create(&hgrid_path).unwrap();
        writeln!(file, "Test grid").unwrap();
        writeln!(file, "2 3").unwrap();
        writeln!(file, "1 0.0 0.0 -10.0").unwrap();
        writeln!(file, "2 1.0 0.0 -12.0").unwrap();
        writeln!(file, "3 0.5 1.0 -8.0").unwrap();
        writeln!(file, "1 3 1 2 3").unwrap();
        writeln!(file, "2 3 1 3 2").unwrap();

        // Load the same hgrid twice
        let hgrid1 = Hgrid::try_from(&hgrid_path).unwrap();
        let hgrid2 = Hgrid::try_from(&hgrid_path).unwrap();

        // Hashes should be identical
        assert_eq!(hgrid1.calculate_hash(), hgrid2.calculate_hash());
        assert_eq!(hgrid1.quick_hash(), hgrid2.quick_hash());
    }

    #[test]
    fn test_hgrid_hash_different_for_different_grids() {
        let temp_dir = tempdir().unwrap();

        // Create first grid
        let hgrid_path1 = temp_dir.path().join("test1.gr3");
        let mut file1 = std::fs::File::create(&hgrid_path1).unwrap();
        writeln!(file1, "Test grid 1").unwrap();
        writeln!(file1, "1 2").unwrap();
        writeln!(file1, "1 0.0 0.0 -10.0").unwrap();
        writeln!(file1, "2 1.0 0.0 -12.0").unwrap();
        writeln!(file1, "1 3 1 2 1").unwrap(); // Different element

        // Create second grid
        let hgrid_path2 = temp_dir.path().join("test2.gr3");
        let mut file2 = std::fs::File::create(&hgrid_path2).unwrap();
        writeln!(file2, "Test grid 2").unwrap();
        writeln!(file2, "1 2").unwrap();
        writeln!(file2, "1 0.0 0.0 -15.0").unwrap(); // Different depth
        writeln!(file2, "2 1.0 0.0 -12.0").unwrap();
        writeln!(file2, "1 3 1 2 1").unwrap();

        let hgrid1 = Hgrid::try_from(&hgrid_path1).unwrap();
        let hgrid2 = Hgrid::try_from(&hgrid_path2).unwrap();

        // Hashes should be different
        assert_ne!(hgrid1.calculate_hash(), hgrid2.calculate_hash());
        assert_ne!(hgrid1.quick_hash(), hgrid2.quick_hash());
    }
}
