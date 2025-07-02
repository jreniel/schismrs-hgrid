use derive_builder::Builder;
use linked_hash_map::LinkedHashMap;
use log;
use proj::Proj;
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
    elements: Option<LinkedHashMap<u32, Vec<u32>>>,
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

    pub fn elements(&self) -> Option<LinkedHashMap<u32, Vec<u32>>> {
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

        let ne = match &self.elements {
            Some(elements) => elements.len(),
            None => 0,
        };

        lines.push(format!("{} {}", ne, self.nodes.len()));
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

        if self.elements.is_some() {
            if let Some(elements) = &self.elements {
                for (local_index, (_element_id, element_indices)) in elements.iter().enumerate() {
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
            }
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

// More idiomatic version using iterator methods
pub fn get_description_without_proj(description: &str) -> String {
    // Early return if the full description is a valid PROJ string
    if Proj::new(description).is_ok() {
        return String::new();
    }

    let words: Vec<&str> = description.split_whitespace().collect();

    // Find the first index where the substring from that point is a valid PROJ string
    if let Some(split_index) = (0..words.len()).find(|&i| {
        let substr = words[i..].join(" ");
        Proj::new(&substr).is_ok()
    }) {
        words[0..split_index].join(" ")
    } else {
        description.to_string()
    }
}

pub fn parse_from_reader<R: Read>(
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

    // When using derive-builder, even Optional must be set explicitly.

    if !elemmap.is_empty() {
        parsed_gr3_builder.elements(elemmap);
    } else {
        parsed_gr3_builder.elements(None);
    }

    if !open_boundaries_vec.is_empty() {
        parsed_gr3_builder.open_boundaries(open_boundaries_vec);
    } else {
        parsed_gr3_builder.open_boundaries(None);
    }

    if !land_boundaries_vec.is_empty() {
        parsed_gr3_builder.land_boundaries(land_boundaries_vec);
    } else {
        parsed_gr3_builder.land_boundaries(None);
    }

    if !interior_boundaries_vec.is_empty() {
        parsed_gr3_builder.interior_boundaries(interior_boundaries_vec);
    } else {
        parsed_gr3_builder.interior_boundaries(None);
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

impl Gr3ParserOutput {
    /// Write the mesh data as a 2DM (SMS) format file
    pub fn write_as_2dm(&self, path: &Path) -> std::io::Result<()> {
        let mut tmpfile = NamedTempFile::new()?;
        log::debug!("Will write 2DM to tmpfile: {:?}", tmpfile);
        writeln!(tmpfile, "{}", self.to_2dm_string())?;
        tmpfile.persist(path)?;
        Ok(())
    }

    /// Convert the mesh data to 2DM format string
    pub fn to_2dm_string(&self) -> String {
        let mut output = String::new();

        // Start with MESH2D header
        output.push_str("MESH2D\n");

        // Add triangular elements (E3T)
        if let Some(elements) = &self.elements {
            for (element_id, element_nodes) in elements.iter() {
                if element_nodes.len() == 3 {
                    output.push_str(&format!(
                        "E3T {} {} {} {}\n",
                        element_id, element_nodes[0], element_nodes[1], element_nodes[2]
                    ));
                }
            }
        }

        // Add quadrilateral elements (E4Q)
        if let Some(elements) = &self.elements {
            for (element_id, element_nodes) in elements.iter() {
                if element_nodes.len() == 4 {
                    output.push_str(&format!(
                        "E4Q {} {} {} {} {}\n",
                        element_id,
                        element_nodes[0],
                        element_nodes[1],
                        element_nodes[2],
                        element_nodes[3]
                    ));
                }
            }
        }

        // Add nodes (ND)
        for (node_id, (coords, values)) in self.nodes.iter() {
            let value = match values {
                Some(v) if !v.is_empty() => v[0], // Use first value if available
                _ => -99999.0,                    // Default value for missing data
            };

            output.push_str(&format!(
                "ND {} {:<.16E} {:<.16E} {:<.16E}\n",
                node_id, coords[0], coords[1], value
            ));
        }

        // Add boundaries
        output.push_str(&self.boundaries_to_2dm_string());

        output
    }

    /// Convert boundaries to 2DM nodestring format
    fn boundaries_to_2dm_string(&self) -> String {
        let mut output = String::new();

        // Process open boundaries
        if let Some(open_boundaries) = &self.open_boundaries {
            for boundary in open_boundaries.iter() {
                if !boundary.is_empty() {
                    output.push_str("NS ");
                    for i in 0..(boundary.len() - 1) {
                        output.push_str(&format!("{} ", boundary[i]));
                    }
                    output.push_str(&format!("-{}\n", boundary[boundary.len() - 1]));
                }
            }
        }

        // Process land boundaries
        if let Some(land_boundaries) = &self.land_boundaries {
            for boundary in land_boundaries.iter() {
                if !boundary.is_empty() {
                    output.push_str("NS ");
                    for i in 0..(boundary.len() - 1) {
                        output.push_str(&format!("{} ", boundary[i]));
                    }
                    output.push_str(&format!("-{}\n", boundary[boundary.len() - 1]));
                }
            }
        }

        // Process interior boundaries
        if let Some(interior_boundaries) = &self.interior_boundaries {
            for boundary in interior_boundaries.iter() {
                if !boundary.is_empty() {
                    output.push_str("NS ");
                    for i in 0..(boundary.len() - 1) {
                        output.push_str(&format!("{} ", boundary[i]));
                    }
                    output.push_str(&format!("-{}\n", boundary[boundary.len() - 1]));
                }
            }
        }

        output
    }
}

#[cfg(test)]
mod tests_2dm {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_2dm_string_generation() {
        // Create a simple test mesh
        let mut nodes = LinkedHashMap::new();
        nodes.insert(1, (vec![0.0, 0.0], Some(vec![-10.5])));
        nodes.insert(2, (vec![1.0, 0.0], Some(vec![-12.3])));
        nodes.insert(3, (vec![0.5, 1.0], Some(vec![-15.7])));
        nodes.insert(4, (vec![1.5, 1.0], Some(vec![-18.2])));

        let mut elements = LinkedHashMap::new();
        elements.insert(1, vec![1, 2, 3]); // Triangle
        elements.insert(2, vec![2, 4, 3]); // Triangle

        let open_boundaries = vec![vec![1, 2]];

        let gr3 = Gr3ParserOutputBuilder::default()
            .description("Test mesh".to_string())
            .nodes(nodes)
            .elements(elements)
            .crs(Some(Arc::new(Proj::new("epsg:6933").unwrap())))
            .open_boundaries(None)
            .land_boundaries(None)
            .interior_boundaries(None)
            .open_boundaries(open_boundaries)
            .build()
            .expect("Failed to build test GR3");

        let sms2dm_string = gr3.to_2dm_string();

        // Verify the output contains expected components
        assert!(sms2dm_string.contains("MESH2D"));
        assert!(sms2dm_string.contains("E3T 1 1 2 3"));
        assert!(sms2dm_string.contains("E3T 2 2 4 3"));
        assert!(sms2dm_string.contains("ND 1"));
        assert!(sms2dm_string.contains("NS 1 -2"));

        println!("Generated 2DM string:\n{}", sms2dm_string);
    }

    #[test]
    fn test_write_2dm_file() {
        // Create a simple test mesh
        let mut nodes = LinkedHashMap::new();
        nodes.insert(1, (vec![0.0, 0.0], Some(vec![-10.5])));
        nodes.insert(2, (vec![1.0, 0.0], Some(vec![-12.3])));
        nodes.insert(3, (vec![0.5, 1.0], Some(vec![-15.7])));

        let mut elements = LinkedHashMap::new();
        elements.insert(1, vec![1, 2, 3]); // Triangle

        let gr3 = Gr3ParserOutputBuilder::default()
            .description("Test mesh".to_string())
            .nodes(nodes)
            .elements(elements)
            .crs(Some(Arc::new(Proj::new("epsg:6933").unwrap())))
            .open_boundaries(None)
            .land_boundaries(None)
            .interior_boundaries(None)
            .build()
            .expect("Failed to build test GR3");

        // Create a temporary directory and file
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test_mesh.2dm");

        // Write the 2DM file
        let result = gr3.write_as_2dm(&file_path);
        assert!(result.is_ok(), "Writing 2DM file should succeed");

        // Verify the file exists and has content
        assert!(file_path.exists(), "2DM file should exist");

        let content =
            std::fs::read_to_string(&file_path).expect("Should be able to read the 2DM file");
        assert!(content.contains("MESH2D"));
        assert!(content.contains("E3T"));
        assert!(content.contains("ND"));

        println!("2DM file written successfully to: {:?}", file_path);
    }

    #[test]
    fn test_mixed_element_2dm() {
        // Create a mesh with both triangles and quads
        let mut nodes = LinkedHashMap::new();
        for i in 1..=6 {
            nodes.insert(i, (vec![i as f64, 0.0], Some(vec![-10.0 - i as f64])));
        }

        let mut elements = LinkedHashMap::new();
        elements.insert(1, vec![1, 2, 3]); // Triangle
        elements.insert(2, vec![2, 4, 5, 3]); // Quad
        elements.insert(3, vec![4, 6, 5]); // Triangle

        let gr3 = Gr3ParserOutputBuilder::default()
            .description("Mixed element mesh".to_string())
            .nodes(nodes)
            .elements(elements)
            .crs(Some(Arc::new(Proj::new("epsg:6933").unwrap())))
            .open_boundaries(None)
            .land_boundaries(None)
            .interior_boundaries(None)
            .build()
            .expect("Failed to build test GR3");

        let sms2dm_string = gr3.to_2dm_string();

        // Verify both element types are present
        assert!(sms2dm_string.contains("E3T 1 1 2 3"));
        assert!(sms2dm_string.contains("E4Q 2 2 4 5 3"));
        assert!(sms2dm_string.contains("E3T 3 4 6 5"));

        println!("Mixed element 2DM string:\n{}", sms2dm_string);
    }
}
