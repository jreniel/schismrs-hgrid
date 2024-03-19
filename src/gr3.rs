use derive_builder::Builder;
use log;
use proj::Proj;
// use std::collections::BTreeMap;
use linked_hash_map::LinkedHashMap;
use std::fmt;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use std::sync::Arc;
use tempfile::NamedTempFile;
use thiserror::Error;
use url::Url;

#[derive(Builder, Default, Debug)]
#[builder(setter(into))]
pub struct Gr3ParserOutput {
    description: Option<String>,
    crs: Option<Arc<Proj>>,
    nodes: LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)>,
    elements: LinkedHashMap<u32, Vec<u32>>, // elements
    open_boundaries: Option<Vec<Vec<u32>>>,
    land_boundaries: Option<Vec<Vec<u32>>>,
    interior_boundaries: Option<Vec<Vec<u32>>>,
}

impl Gr3ParserOutput {
    pub fn nodes(&self) -> LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        self.nodes.clone()
    }

    pub fn nodes_values_reversed_sign(&self) -> LinkedHashMap<u32, (Vec<f64>, Option<Vec<f64>>)> {
        let mut new_nodes = LinkedHashMap::<u32, (Vec<f64>, Option<Vec<f64>>)>::new();
        for (&node_id, (coord, value)) in self.nodes.iter() {
            let reversed_value = value.as_ref().map(|v| v.iter().map(|&x| -x).collect());

            new_nodes.insert(node_id, (coord.clone(), reversed_value));
        }
        new_nodes
    }

    pub fn elements(&self) -> LinkedHashMap<u32, Vec<u32>> {
        self.elements.clone()
    }
    pub fn crs(&self) -> Option<Arc<Proj>> {
        self.crs.clone()
    }
    pub fn description(&self) -> Option<String> {
        self.description.clone()
    }

    pub fn open_boundaries(&self) -> Option<Vec<Vec<u32>>> {
        self.open_boundaries.clone()
    }

    pub fn land_boundaries(&self) -> Option<Vec<Vec<u32>>> {
        self.land_boundaries.clone()
    }

    pub fn interior_boundaries(&self) -> Option<Vec<Vec<u32>>> {
        self.interior_boundaries.clone()
    }

    pub fn get_full_string(&self) -> String {
        let mut lines = Vec::new();
        lines.push(self.description().unwrap_or("".to_owned()));
        lines.join("\n")
    }
}

impl fmt::Display for Gr3ParserOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = Vec::new();
        let crs_str: String = self
            .crs
            .as_ref()
            .map(|proj| proj.proj_info().definition.clone().unwrap_or_default())
            .unwrap_or_default();

        let desc_str = self.description.as_ref().map_or("", String::as_str);

        if crs_str.is_empty() && desc_str.is_empty() {
            lines.push("".to_string());
        } else if crs_str.is_empty() {
            lines.push(desc_str.to_string());
        } else if desc_str.is_empty() {
            lines.push(crs_str.to_string());
        } else {
            lines.push(format!("{} {}", crs_str, desc_str));
        };

        lines.push(format!("{} {}", self.elements.len(), self.nodes.len()));
        let mut fort_index_from_node_id = LinkedHashMap::new();
        for (local_index, (&node_id, (coord, value))) in self.nodes.iter().enumerate() {
            let fortran_index = local_index + 1;
            fort_index_from_node_id.insert(node_id, fortran_index);
            let value_str = match value {
                Some(v) => v
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
                None => "-99999.".to_string(),
            };

            lines.push(format!(
                "{} {} {} {}",
                fortran_index, coord[0], coord[1], value_str
            ));
        }

        for (local_index, (_element_id, element_indices)) in self.elements.iter().enumerate() {
            let fortran_index = local_index + 1;

            // Translate element node indices to their corresponding Fortran indices and build the element_str
            let element_str = element_indices
                .iter()
                .map(|&element_index| {
                    fort_index_from_node_id
                        .get(&element_index)
                        .expect("Expected node ID in map")
                        .to_string()
                })
                .collect::<Vec<String>>()
                .join(" ");

            lines.push(format!(
                "{} {} {}",
                fortran_index,
                element_indices.len(),
                element_str
            ));
        }
        if self.open_boundaries.is_some()
            || self.land_boundaries.is_some()
            || self.interior_boundaries.is_some()
        {
            // Handle open_boundaries if it's Some
            if let Some(open) = &self.open_boundaries {
                lines.push(format!("{} ! total number of open boundaries", open.len()));
                let mut total_number_of_open_boundary_nodes = 0;
                for this_open_bound in open.iter() {
                    total_number_of_open_boundary_nodes += this_open_bound.len();
                }
                lines.push(format!(
                    "{} ! total number of open boundary nodes",
                    total_number_of_open_boundary_nodes
                ));
                for (local_index, this_open_bound) in open.iter().enumerate() {
                    let fortran_index = local_index + 1;
                    lines.push(format!(
                        "{} ! number of nodes for ocean_boundary_{}",
                        this_open_bound.len(),
                        fortran_index
                    ));
                    for this_open_bound_index in this_open_bound.iter() {
                        let this_open_bound_fortran_index =
                            fort_index_from_node_id[this_open_bound_index];
                        lines.push(format!("{}", this_open_bound_fortran_index));
                    }
                }
            } else {
                lines.push("0 ! total number of open boundaries".to_owned());
                lines.push("0 ! total number of open boundary nodes".to_owned());
            }

            let mut total_number_of_non_ocean_boundaries = 0;
            let mut total_number_of_non_ocean_boundaries_nodes = 0;

            if let Some(land) = &self.land_boundaries {
                for land_bnd in land.iter() {
                    total_number_of_non_ocean_boundaries += 1;
                    total_number_of_non_ocean_boundaries_nodes += land_bnd.len();
                }
            }

            if let Some(interior) = &self.interior_boundaries {
                for interior_bnd in interior.iter() {
                    total_number_of_non_ocean_boundaries += 1;
                    total_number_of_non_ocean_boundaries += interior_bnd.len();
                }
            }
            lines.push(format!(
                "{} ! total number of non-ocean boundaries",
                total_number_of_non_ocean_boundaries
            ));
            lines.push(format!(
                "{} ! total number of non-ocean boundaries nodes",
                total_number_of_non_ocean_boundaries_nodes
            ));

            if let Some(land) = &self.land_boundaries {
                for (local_index, this_land_bound) in land.iter().enumerate() {
                    let fortran_index = local_index + 1;
                    lines.push(format!(
                        "{} ! number of nodes for land_boundary_{}",
                        this_land_bound.len(),
                        fortran_index
                    ));
                    for this_land_bound_index in this_land_bound.iter() {
                        let this_land_bound_fortran_index =
                            fort_index_from_node_id[this_land_bound_index];
                        lines.push(format!("{}", this_land_bound_fortran_index));
                    }
                }
            };

            if let Some(interior) = &self.interior_boundaries {
                for (local_index, this_interior_bound) in interior.iter().enumerate() {
                    let fortran_index = local_index + 1;
                    lines.push(format!(
                        "{} ! number of nodes for interior_boundary_{}",
                        this_interior_bound.len(),
                        fortran_index
                    ));
                    for this_interior_bound_index in this_interior_bound.iter() {
                        let this_interior_bound_fortran_index =
                            fort_index_from_node_id[this_interior_bound_index];
                        lines.push(format!("{}", this_interior_bound_fortran_index));
                    }
                }
            }
        }
        write!(f, "{}", lines.join("\n"))
    }
}

#[derive(Error, Debug)]
pub enum Gr3ParserError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Empty file error: {0}")]
    EmptyFile(String),

    #[error("Line read error: file {0}, error: {1}")]
    LineReadError(String, String),

    #[error("Error requesting hgrid from URL: {0}, error: {1}")]
    RequestFromUrlError(String, String),

    #[error(transparent)]
    Gr3ParserOutputBuilderError(#[from] Gr3ParserOutputBuilderError),
}

pub fn parse_from_path_ref(path: &Path) -> Result<Gr3ParserOutput, Gr3ParserError> {
    let fname = &path.display().to_string();
    let file = match File::open(&fname) {
        Ok(file) => file,
        Err(e) => {
            return Err(Gr3ParserError::IoError(format!(
                "Failed to open {}: {}",
                fname, e
            )));
        }
    };
    let reader = BufReader::new(file);
    parse_from_reader(reader, fname)
}

pub fn parse_from_url(url: &Url) -> Result<Gr3ParserOutput, Gr3ParserError> {
    let response = reqwest::blocking::get(url.to_string())
        .map_err(|err| Gr3ParserError::RequestFromUrlError(url.to_string(), err.to_string()))?;
    // Read the response body as a String
    let body = response
        .text()
        .map_err(|err| Gr3ParserError::RequestFromUrlError(url.to_string(), err.to_string()))?;
    let reader = BufReader::new(body.as_bytes());
    parse_from_reader(reader, &url.to_string())
}

fn get_proj_from_description(description: &str) -> Option<Proj> {
    if let Ok(proj) = Proj::new(description) {
        return Some(proj);
    }

    let words: Vec<&str> = description.split_whitespace().collect();
    for i in 0..words.len() {
        let substr = words[i..].join(" ");
        if let Ok(proj) = Proj::new(&substr) {
            return Some(proj);
        }
    }

    None
}

pub fn get_description_without_proj(description: &str) -> String {
    if Proj::new(description).is_ok() {
        return String::new();
    }

    let words: Vec<&str> = description.split_whitespace().collect();
    for i in 0..words.len() {
        let substr = words[i..].join(" ");
        if Proj::new(&substr).is_ok() {
            return words[0..i].join(" ");
        }
    }
    description.to_string() // If no Proj found, return the original description.
}

fn parse_from_reader<R: Read>(
    reader: BufReader<R>,
    fname: &str, // Passed separately for error messages
) -> Result<Gr3ParserOutput, Gr3ParserError> {
    let mut buf = reader.lines();
    let description_raw_str: String = match buf.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                e.to_string(),
            ));
        }
        None => return Err(Gr3ParserError::EmptyFile(fname.to_string())),
    };
    let description = get_description_without_proj(&description_raw_str);
    let crs = get_proj_from_description(&description_raw_str).map(Arc::new);
    let line = match buf.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                e.to_string(),
            ));
        }
        None => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                "Expected second line to contain NE NP but it's empty.".to_string(),
            ))
        }
    };
    let mut line = line.split_whitespace();
    log::info!("Start reading nodes...");
    let ne: u32 = match line.next() {
        Some(ne_str) => match ne_str.parse::<u32>() {
            Ok(ne) => ne,
            Err(_) => {
                return Err(Gr3ParserError::LineReadError(fname.to_string(),
                    format!("Expected first item in second line (number of elements NE) to be castable to an u32 but found {}.", ne_str)
                    ))
            }
        },
        None => {
            return Err(
                Gr3ParserError::LineReadError(fname.to_string(),
                "Expected second line to contain NE NP but it's empty.".to_string(),
                    ))
        }
    };
    let np: u32 = match line.next() {
        Some(np_str) => match np_str.parse::<u32>() {
            Ok(np) => np,
            Err(_) => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Error reading gr3 file: {}. Expected second item in second line (number of nodes NP) to be castable to an u32 but found {}.", fname, np_str))
                    );
            }
        },
        None => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                "Expected second line to contain two numbers (NE NP) but found only one."
                    .to_string(),
            ));
        }
    };
    let mut nodemap = LinkedHashMap::new();
    for _ in 0..np {
        let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!(
                        "Expected {} lines with node data but found only {}.",
                        np,
                        nodemap.len()
                    ),
                ))
            }
        };
        let mut line = line.split_whitespace();
        let node_id: u32 =  match line.next() {
            Some(node_id_str) => match node_id_str.parse::<u32>() {
                Ok(node_id) => node_id,
                Err(_) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected first item in line {} (node id) to be castable to an u32 but found {}.", nodemap.len() + 1, node_id_str)
                            )
                        )
                }
            },
            None => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Expected line {} to contain node data but found an empty line.", nodemap.len() + 1)
                        )
                    )
            }
        };
        let mut coords = Vec::new();
        let x: f64 = match line.next() {
            Some(x_str) => match x_str.parse::<f64>() {
                Ok(x) => x,
                Err(_) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected second item in line {} (node x coordinate) to be castable to an f64 but found {}.", nodemap.len() + 1, x_str)
                            )
                        )
                }
            },
            None => {
                return Err(
                    Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected line {} to contain node data but found only one item.", nodemap.len() + 1)
                        )
                    )
            }
        };
        let y: f64 = match line.next() {
            Some(y_str) => match y_str.parse::<f64>() {
                Ok(y) => y,
                Err(_) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected third item in line {} (node y coordinate) to be castable to an f64 but found {}.", nodemap.len() + 1, y_str)
                            )
                        )
                }
            },
            None => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Expected line {} to contain node data but found only two items.", nodemap.len() + 1)
                        )

                    )
            }
        };
        coords.push(x);
        coords.push(y);
        let mut values = Vec::new();
        for val in line {
            let val: f64 = match val.parse() {
                Ok(val) => val,
                Err(_) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected item in line {} (node value) to be castable to an f64 but found {}.", nodemap.len() + 1, val)
                        ));
                }
            };
            values.push(val);
        }
        let data = (coords, Some(values));
        nodemap.insert(node_id, data);
    }
    // let nodes = Nodes::new(nodemap, crs);
    log::info!("Start reading elements...");
    let mut elemmap = LinkedHashMap::new();
    for _ in 0..ne {
        let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!(
                        "Expected {} lines with element data but found only {}.",
                        ne,
                        elemmap.len()
                    ),
                ))
            }
        };
        let mut line = line.split_whitespace();
        let element_id: u32 = match line.next() {
                Some(element_id_str) => match element_id_str.parse::<u32>() {
                    Ok(element_id) => element_id,
                    Err(_) => {
                        return Err(
                            Gr3ParserError::LineReadError(
                                fname.to_string(),
                                format!("Expected first item in line {} (element id) to be castable to an u32 but found {}.", elemmap.len() + 1, element_id_str)
                                )
                            )
                    }
                },
                None => {
                    return Err(
                        Gr3ParserError::LineReadError(fname.to_string(),
                        format!("Expected line {} to contain element data but found an empty line.", elemmap.len() + 1)
                            )
                        )
                }
            };
        let element_len: u8 = match line.next() {
                Some(element_len_str) => match element_len_str.parse::<u8>() {
                    Ok(element_len) => element_len,
                    Err(_) => {
                        return Err(
                            Gr3ParserError::LineReadError(
                                fname.to_string(),
                                format!("Expected second item in line {} (element length) to be castable to an u8 but found {}.", elemmap.len() + 1, element_len_str)
                                )
                            )
                    }
                },
                None => {
                    return Err(
                        Gr3ParserError::LineReadError(fname.to_string(),
                        format!("Expected line {} to contain element data but found only one item.", elemmap.len() + 1)
                            )
                        )
                }
            };
        let mut element_vec = Vec::<u32>::new();
        for _ in 0..element_len {
            let line = match line.next() {
                Some(line) => match line.parse::<u32>() {
                    Ok(line) => line,
                    Err(_) => {
                        return Err(
                            Gr3ParserError::LineReadError(
                                fname.to_string(),
                                format!("Expected item in line {} (element node id) to be castable to an u32 but found {}.", elemmap.len() + 1, line)
                            )
                        )
                    }
                },
                None => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected line {} to contain element data but found only {} items.", elemmap.len() + 1, element_vec.len())
                            )
                        )
                }
            };
            element_vec.push(line);
        }
        elemmap.insert(element_id, element_vec);
    }
    log::debug!("Done reading elements!");
    // let elements = Elements::new(&nodes, elemmap).map_err(|e| Gr3ParserError::ElementsConstructorError(e))?;
    // log::debug!("Done crating elements object");
    // parse boundaries
    let line = match buf.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                e.to_string(),
            ));
        }
        None => {
            let mut parsed_gr3_builder = Gr3ParserOutputBuilder::default();
            parsed_gr3_builder.description(description);
            parsed_gr3_builder.nodes(nodemap);
            if !elemmap.is_empty() {
                parsed_gr3_builder.elements(elemmap);
            }
            return Ok(parsed_gr3_builder.build()?);
        }
    };
    let first_word = line.split_whitespace().next();
    let number_of_open_boundaries: u32 = match first_word {
        Some(first_word) => match first_word.parse::<u32>() {
            Ok(number_of_open_boundaries) => number_of_open_boundaries,
            Err(_) => {
                return Err(
                    Gr3ParserError::LineReadError(fname.to_string(),
                    format!("Expected first line after element data to contain the number of open boundaries (u32) but found {}.", first_word),
                        )
                    )
            },
        },
        None => {
            return Err(
                Gr3ParserError::LineReadError(
                    fname.to_string(),
                    "Expected first line after element data to contain the number of open boundaries (u32) but found an empty line.".to_string(),
                    )
                )
        },
    };
    let line = match buf.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                e.to_string(),
            ));
        }
        None => {
            return Err(
                Gr3ParserError::LineReadError(
                    fname.to_string(),
                    "Expected second line after element data to contain the total number of open boundary nodes but found empty line.".to_string(),
                        )
                )
        }
    };
    let first_word = line.split_whitespace().next();
    let _total_number_of_open_boundary_nodes: u32 = match first_word {
        Some(first_word) => match first_word.parse::<u32>() {
            Ok(total_number_of_open_boundary_nodes) => total_number_of_open_boundary_nodes,
            Err(_) => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Expected second line after element data to contain the total number of open boundary nodes (u32) but found {}.", first_word)
                        )
                    )
            }
        },
        None => {
            return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        "Expected second line after element data to contain the total number of open boundary nodes (u32) but found an empty line.".to_string(),
                        )
                )
        }
    };
    let mut open_boundaries_vec = Vec::<Vec<u32>>::new();
    for _ in 0..number_of_open_boundaries {
        let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!(
                        "Expected {} lines with open boundary data but found only {}.",
                        number_of_open_boundaries,
                        open_boundaries_vec.len()
                    ),
                ))
            }
        };
        let mut line = line.split_whitespace();
        let number_of_nodes_for_this_boundary = match line.next() {
            Some(number_of_nodes_for_this_boundary_str) => match number_of_nodes_for_this_boundary_str.parse::<u32>() {
                Ok(number_of_nodes_for_this_boundary) => number_of_nodes_for_this_boundary,
                Err(_) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected first item in line {} (number of nodes for this boundary) to be castable to an u32 but found {}.", open_boundaries_vec.len() + 1, number_of_nodes_for_this_boundary_str)
                            )
                        )
                }
            },
            None => {
                return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected line {} to contain open boundary data but found an empty line.", open_boundaries_vec.len() + 1)
                            )
                        )
            }
        };
        let mut boundary_vec = Vec::<u32>::new();
        for _ in 0..number_of_nodes_for_this_boundary {
            let line = match buf.next() {
                Some(Ok(line)) => match line.split_whitespace().next() {
                    Some(number_str) => match number_str.parse::<u32>() {
                        Ok(number) => number,
                        Err(_) => {
                            return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected item in line {} (open boundary node id) to be castable to an u32 but found {}.", open_boundaries_vec.len() + 1, line),
                            )
                                )
                        }
                    },
                    None => {
                        return Err(
                            Gr3ParserError::LineReadError(
                                fname.to_string(),
                                format!("Expected item in line {} to be an u32 but found an empty string.", open_boundaries_vec.len() + 1),
                                )
                            )
                    },
                },
                Some(Err(e)) => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            e.to_string()
                            )
                        )
                },
                None => {
                    return Err(
                        Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Expected open boundary {} to contain open boundary data but found only {} items.", open_boundaries_vec.len() + 1, boundary_vec.len()),
                            )
                        )
                }
            };
            boundary_vec.push(line);
        }
        open_boundaries_vec.push(boundary_vec);
    }
    // parse land boundaries
    let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    "Expected line after open boundary data to contain the number of land boundaries but found empty line.".to_string(),
                ))
            }
    };
    let first_word = line.split_whitespace().next();
    let number_of_land_boundaries: u32 = match first_word {
        Some(first_word) => match first_word.parse::<u32>() {
            Ok(number_of_land_boundaries) => number_of_land_boundaries,
            Err(_) => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Expected line after open boundary data to contain the number of land boundaries (u32) but found {}.", first_word)
                        )
                    )
            }
        },
        None => {
                return Err(
                    Gr3ParserError::LineReadError(
                        fname.to_string(),
                        "Expected line after open boundary data to contain the number of land boundaries (u32) but found an empty line.".to_string(),
                        )
                    )
        }
    };

    let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    "Expected line after number of land boundaries to contain the total number of land boundary nodes but found empty line.".to_string(),
                ))
            }
    };
    let first_word = line.split_whitespace().next();
    let _total_number_of_land_boundary_nodes: u32 = match first_word {
        Some(first_word) => match first_word.parse::<u32>() {
            Ok(total_number_of_land_boundary_nodes) => total_number_of_land_boundary_nodes,
            Err(_) => {
                return Err(
                    Gr3ParserError::LineReadError(fname.to_string(),
                    format!("Expected line after number of land boundaries to contain the total number of land boundary nodes (u32) but found {}.", first_word)
                        )
                    )
            }
        },
        None => {
                return Err(
                    Gr3ParserError::LineReadError(fname.to_string(),
                    "Error reading gr3 file: {}. Expected line after number of land boundaries to contain the total number of land boundary nodes (u32) but found an empty line.".to_string(),
                    )
                )
        }
    };
    let mut land_boundaries_vec = Vec::<Vec<u32>>::new();
    let mut interior_boundaries_vec = Vec::<Vec<u32>>::new();
    for _ in 0..number_of_land_boundaries {
        let line = match buf.next() {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    e.to_string(),
                ));
            }
            None => {
                return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!(
                        "Expected {} lines with land boundary data but found only {}.",
                        number_of_land_boundaries,
                        land_boundaries_vec.len()
                    ),
                ))
            }
        };
        let mut split_line = line.split_whitespace();

        let number_of_nodes_for_this_boundary = match split_line.next() {
            Some(value) => match value.parse::<u64>() {
                Ok(value) => value,
                Err(_) => return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!("Error reading gr3 file: {}. Expected first item in line to be castable to an u64.", fname)
                ))
            },
            None => return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                format!("Error reading gr3 file: {}. Expected line to contain number_of_nodes_for_this_boundary but found an empty line.", fname)
            ))
        };

        let boundary_id_type = match split_line.next() {
            Some(value) => match value.parse::<u8>() {
                Ok(value) => value,
                Err(_) => return Err(Gr3ParserError::LineReadError(
                    fname.to_string(),
                    format!("Error reading gr3 file: {}. Expected second item in line to be castable to a u8.", fname)
                ))
            },
            None => return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                format!("Error reading gr3 file: {}. Expected line to contain boundary_id_type but found an empty line.", fname)
            ))
        };

        let mut this_boundary_vec = Vec::<u32>::new();
        for _ in 0..number_of_nodes_for_this_boundary {
            let line = match buf.next() {
                Some(Ok(line)) => line,
                Some(Err(e)) => {
                    return Err(Gr3ParserError::LineReadError(
                        fname.to_string(),
                        e.to_string(),
                    ));
                }
                None => {
                    return Err(Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!(
                            "Expected {} lines with land boundary data but found only {}.",
                            number_of_land_boundaries,
                            land_boundaries_vec.len()
                        ),
                    ))
                }
            };
            let mut split_line = line.split_whitespace();
            let node_id = match split_line.next() {
                    Some(value) => match value.parse::<u32>() {
                        Ok(value) => value,
                        Err(_) => return Err(Gr3ParserError::LineReadError(
                            fname.to_string(),
                            format!("Error reading gr3 file: {}. Expected first item in line to be castable to an u32.", fname)
                        ))
                    },
                    None => return Err(Gr3ParserError::LineReadError(
                        fname.to_string(),
                        format!("Error reading gr3 file: {}. Expected line to contain node_id but found an empty line.", fname)
                    ))
                };
            this_boundary_vec.push(node_id);
        }
        if boundary_id_type == 0 {
            land_boundaries_vec.push(this_boundary_vec);
        } else if boundary_id_type == 1 {
            interior_boundaries_vec.push(this_boundary_vec);
        } else {
            return Err(Gr3ParserError::LineReadError(
                fname.to_string(),
                format!("Error reading gr3 file: {}. Expected boundary_id_type to be 0 or 1 but found {}.", fname, boundary_id_type)
            ));
        }
    }

    let mut parsed_gr3_builder = Gr3ParserOutputBuilder::default();
    parsed_gr3_builder.description(description);
    parsed_gr3_builder.nodes(nodemap);
    parsed_gr3_builder.crs(crs);

    if !elemmap.is_empty() {
        parsed_gr3_builder.elements(elemmap);
    }

    if !open_boundaries_vec.is_empty() {
        parsed_gr3_builder.open_boundaries(open_boundaries_vec);
    }

    if !land_boundaries_vec.is_empty() {
        parsed_gr3_builder.land_boundaries(land_boundaries_vec);
    }

    if !interior_boundaries_vec.is_empty() {
        parsed_gr3_builder.interior_boundaries(interior_boundaries_vec);
    }
    log::debug!("Done with parsing full file!");
    Ok(parsed_gr3_builder.build()?)
}

pub fn write_to_path(path: &Path, gr3: &Gr3ParserOutput) -> std::io::Result<()> {
    let mut tmpfile = NamedTempFile::new()?;
    log::debug!("Will write to tmpfile: {:?}", tmpfile);
    writeln!(tmpfile, "{}", gr3)?;
    tmpfile.persist(path)?;
    Ok(())
}
