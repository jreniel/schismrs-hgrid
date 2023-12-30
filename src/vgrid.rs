use derive_builder::Builder;
use std::io::{self, prelude::*, BufReader};
use std::{fs::File, path::PathBuf};

pub trait Vgrid {
    fn nlevels(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct SLevels {
    levels: Vec<f64>,
}

impl SLevels {
    pub fn levels(&self) -> &Vec<f64> {
        &self.levels
    }
    pub fn nlevels(&self) -> usize {
        self.levels.len()
    }
}

impl TryFrom<Vec<f64>> for SLevels {
    type Error = Box<dyn std::error::Error>;
    fn try_from(value: Vec<f64>) -> Result<SLevels, Box<dyn std::error::Error>> {
        if value[0] != -1.0 || *value.last().unwrap() != 0.0 {
            Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "Array s_levels must start with -1 and end with 0",
            )))
        } else {
            Ok(Self { levels: value })
        }
    }
}

impl From<usize> for SLevels {
    fn from(value: usize) -> Self {
        let start = -1.0;
        let stop = 0.0;
        let step = (stop - start) / (value - 1) as f64;
        let levels: Vec<f64> = (0..value).map(|i| start + i as f64 * step).collect();
        Self { levels }
    }
}

#[derive(Debug, Clone)]
pub struct ZLevels {
    levels: Vec<f64>,
}

impl ZLevels {
    pub fn levels(&self) -> &Vec<f64> {
        &self.levels
    }
    pub fn nlevels(&self) -> usize {
        self.levels.len()
    }
}

impl TryFrom<Vec<f64>> for ZLevels {
    type Error = Box<dyn std::error::Error>;
    fn try_from(levels: Vec<f64>) -> Result<ZLevels, Box<dyn std::error::Error>> {
        if levels.iter().any(|&x| x > 0.0) {
            Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "z_levels must be < 0",
            )))
        } else if levels.len() > 1 {
            for i in 0..levels.len() - 1 {
                if levels[i] >= levels[i + 1] {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "z_levels must be increasing and all < 0",
                    )));
                }
            }
            Ok(Self { levels })
        } else {
            Ok(Self { levels })
        }
    }
}

#[derive(Debug, Clone, Builder)]
#[builder(build_fn(validate = "Self::validate"))]
#[builder(setter(strip_option))]
pub struct SZ {
    slevels: SLevels,
    zlevels: Option<ZLevels>,
    theta_f: Option<f64>,
    theta_b: Option<f64>,
    hc: Option<f64>,
}

impl SZBuilder {
    fn validate(&self) -> Result<(), String> {
        if let Some(ref theta_b) = self.theta_b {
            if *theta_b <= Some(0.0) {
                return Err(format!(
                    "theta_b must be a positive number but got {}",
                    theta_b.unwrap()
                )
                .into());
            }
            if *theta_b >= Some(1.0) {
                return Err(
                    format!("theta_b must be less than 1.0 but got {}", theta_b.unwrap()).into(),
                );
            }
        }

        Ok(())
    }
}

impl SZ {
    pub fn slevels(&self) -> &SLevels {
        &self.slevels
    }

    pub fn zlevels(&self) -> Option<&ZLevels> {
        self.zlevels.as_ref()
    }

    pub fn theta_f(&self) -> Option<f64> {
        self.theta_f
    }

    pub fn theta_b(&self) -> Option<f64> {
        self.theta_b
    }

    pub fn hc(&self) -> Option<f64> {
        self.hc
    }

    pub fn from_pathbuf_ref(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut buf = reader.lines();
        let line: String = match buf.next() {
            Some(line) => line?,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("File {} is empty", path.to_str().unwrap_or("Unknown")),
                )))
            }
        };

        let mut words = line.split_whitespace();
        let ivcor: u8 = match words.next() {
            Some(word) => match word.parse::<u8>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected first item in line 1 of file {} to be of type u8 but got {}",
                            path.display(),
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected one item in line 1 of file {}, but found none",
                        path.display()
                    ),
                )))
            }
        };
        if ivcor != 2 {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Expected first item in line 1 of file {} to be 2 (SZ vgrid) but got {}",
                    path.display(),
                    ivcor
                ),
            )));
        };

        let line: String = match buf.next() {
            Some(line) => line?,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Expected line 2 of file {} but found none", path.display()),
                )))
            }
        };

        let mut words = line.split_whitespace();

        let nvrt: u8 = match words.next() {
            Some(word) => match word.parse::<u8>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected first item in line 2 of file {} to be of type u8 but got {}",
                            path.display(),
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected three items in line 2 of file {}, but found none",
                        path.display()
                    ),
                )))
            }
        };

        let kz: u8 = match words.next() {
            Some(word) => match word.parse::<u8>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected second item in line 2 of file {} to be u8 but got {}",
                            path.display(),
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected three items in line 2 of file {}, but found only one",
                        path.display()
                    ),
                )))
            }
        };

        let h_s: f64 = match words.next() {
            Some(word) => match word.parse::<f64>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected third item in line 2 of file {} to be f64 but got {}",
                            path.display(),
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected three items in line 2 of file {}, but found only two",
                        path.display()
                    ),
                )))
            }
        };

        // if h_s is negative, print a warning:
        if h_s < 0f64 {
            eprintln!(
                "WARNING: h_s is negative in line 2 of file {}. Expected a strictly positive float.",
                path.display()
            );
        } else if h_s == 0f64 {
            eprintln!(
                "WARNING: h_s is zero in line 2 of file {}. Expected a strictly positive float.",
                path.display()
            );
        }

        // this line just reads Z levels and can be ignored
        let _line: String = match buf.next() {
            Some(line) => line?,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Expected line 3 of file {} but found none", path.display()),
                )))
            }
        };

        let mut previous_sequence_number: Option<u8> = None;
        let mut previous_value: Option<f64> = None;
        let mut z_levels: Vec<f64> = Vec::new();
        for _ in 0..kz {
            let line = match buf.next() {
                Some(Ok(line)) => line,
                Some(Err(e)) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Error reading line: {}", e),
                    )))
                }
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "Unexpected end of file",
                    )))
                }
            };

            let mut words = line.split_whitespace();

            let sequence_number: u8 = match words.next() {
                Some(word) => match word.parse::<u8>() {
                    Ok(num) => num,
                    Err(_) => {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::Other,
                            format!("Expected first item in line to be u8 but got {}", word),
                        )))
                    }
                },
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "Expected two items in line, but found none",
                    )))
                }
            };

            let value: f64 = match words.next() {
                Some(word) => match word.parse::<f64>() {
                    Ok(num) => num,
                    Err(_) => {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::Other,
                            format!("Expected second item in line to be f64 but got {}", word),
                        )))
                    }
                },
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "Expected two items in line, but found only one",
                    )))
                }
            };

            if let Some(prev_seq_num) = previous_sequence_number {
                if sequence_number != prev_seq_num + 1 {
                    eprintln!(
                        "Warning: Expected sequence number {} but got {}",
                        prev_seq_num + 1,
                        sequence_number
                    );
                }
            }
            previous_sequence_number = Some(sequence_number);

            if let Some(prev_value) = previous_value {
                if value <= prev_value {
                    eprintln!(
                        "Warning: Expected a value greater than {} but got {}",
                        prev_value, value
                    );
                }
            }
            z_levels.push(value);
            previous_value = Some(value);
        }

        // if last value of z_levels is not equal to -h_s, print a warning:
        if z_levels[z_levels.len() - 1] != -h_s {
            eprintln!(
                "WARNING: last value of z_levels is not equal to -h_s in line 2 of file {}. Expected {} but got {}.",
                path.display(),
                -h_s,
                z_levels[z_levels.len() - 1]
            );
        }

        // next line just reads S Levels and can be ignored
        let _line: String = match buf.next() {
            Some(line) => line?,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected line {} of file {} but found none",
                        kz + 3,
                        path.display()
                    ),
                )))
            }
        };
        // next line reads: 30. 0.7 5.  !h_c, theta_b, theta_f
        let line: String = match buf.next() {
            Some(line) => line?,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Expected line {} of file {} but found none",
                        kz + 4,
                        path.display()
                    ),
                )))
            }
        };
        let mut words = line.split_whitespace();
        let hc: f64 = match words.next() {
            Some(word) => match word.parse::<f64>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected first item in line {} to be f64 but got {}",
                            kz + 4,
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    "Expected three items in line, but found none",
                )))
            }
        };
        let theta_b: f64 = match words.next() {
            Some(word) => match word.parse::<f64>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected second item in line {} to be f64 but got {}",
                            kz + 4,
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    "Expected three items in line, but found only one",
                )))
            }
        };
        // warn if theta_b is not between 0 and 1
        if theta_b < 0. || theta_b > 1. {
            eprintln!(
                "WARNING: theta_b is not between 0 and 1 in line {} of file {}. Expected {} but got {}.",
                kz+4,
                path.display(),
                0.,
                theta_b
            );
        }
        let theta_f: f64 = match words.next() {
            Some(word) => match word.parse::<f64>() {
                Ok(num) => num,
                Err(_) => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected third item in line {} to be f64 but got {}",
                            kz + 4,
                            word
                        ),
                    )))
                }
            },
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    "Expected three items in line, but found only two",
                )))
            }
        };
        // warn if theta_f < 0
        if theta_f < 0. {
            eprintln!(
                "WARNING: theta_f is less than 0 in line {} of file {}. Expected {} but got {}.",
                kz + 4,
                path.display(),
                0.,
                theta_f
            );
        }
        let mut s_levels: Vec<f64> = Vec::new();
        let mut previous_value: Option<f64> = None;
        let mut previous_sequence_number: Option<usize> = None;
        let n_slevels = nvrt - kz + 1;
        for this_level in 0..n_slevels {
            let line = match buf.next() {
                Some(line) => line?,
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Expected line {} of file {} but found none",
                            kz + 5,
                            path.display()
                        ),
                    )))
                }
            };
            let mut words = line.split_whitespace();
            let mut sequence_number: usize = match words.next() {
                Some(word) => match word.parse::<usize>() {
                    Ok(num) => num,
                    Err(_) => {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "Expected first item in line {} to be usize but got {}",
                                kz + 5,
                                word
                            ),
                        )))
                    }
                },
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "Expected two items in line, but found none",
                    )))
                }
            };
            let value: f64 = match words.next() {
                Some(word) => match word.parse::<f64>() {
                    Ok(num) => num,
                    Err(_) => {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "Expected second item in line {} to be f64 but got {}",
                                kz + 5,
                                word
                            ),
                        )))
                    }
                },
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        "Expected two items in line, but found only one",
                    )))
                }
            };
            // warn if _ == 0 and sequence_number != 1
            // this is because the example may have a minor error?
            if sequence_number != 1 && this_level == 0 {
                eprintln!(
                    "WARNING: sequence_number is not 1 in line {} of file {}. Expected {} but got {}.",
                    kz + 5,
                    path.display(),
                    1,
                    sequence_number
                );
                sequence_number = 1;
            }
            // warn if sequence_number != previous_sequence_number + 1
            if let Some(prev_seq_num) = previous_sequence_number {
                if sequence_number != prev_seq_num + 1 {
                    eprintln!(
                        "WARNING: Expected sequence number {} but got {} in line {} of file {}",
                        prev_seq_num + 1,
                        sequence_number,
                        kz + 5,
                        path.display()
                    );
                }
            }
            previous_sequence_number = Some(sequence_number);

            if let Some(prev_value) = previous_value {
                if value <= prev_value {
                    eprintln!(
                        "Warning: Expected a value greater than {} but got {}",
                        prev_value, value
                    );
                }
            }
            s_levels.push(value);
            previous_value = Some(value);
        }
        let s_levels = SLevels::try_from(s_levels)?;
        let z_levels = ZLevels::try_from(z_levels)?;
        let sz = SZBuilder::default()
            .zlevels(z_levels)
            .slevels(s_levels)
            .theta_b(theta_b)
            .theta_f(theta_f)
            .hc(hc)
            .build()?;
        Ok(sz)
    }
}

impl Vgrid for SZ {
    fn nlevels(&self) -> usize {
        let mut nlevels = self.slevels.nlevels();
        if let Some(zlevels) = &self.zlevels {
            // -1 because the first level is shared with the s-levels
            nlevels += zlevels.nlevels() - 1;
        }
        nlevels
    }
}

#[derive(Debug, Clone)]
pub enum VgridType {
    SZ(SZ),
    // Z,
}

pub fn from_pathbuf_ref(path: &PathBuf) -> Result<Box<dyn Vgrid>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut buf = reader.lines();
    let line: String = match buf.next() {
        Some(line) => line?,
        None => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Error reading vgrid file: {} is an empty file",
                    path.display()
                ),
            )))
        }
    };
    let first_word = line.split_whitespace().next();
    let ivcor: u8 = match first_word {
        Some(first_word) => match first_word.parse::<u8>() {
            Ok(ivcor) => ivcor,
            Err(_) => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error reading vgrid file: {}. Expected first line to contain ivcor (1 or 2) but found {}.", path.display(), first_word)
                )))
            }
        },
        None => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Error reading vgrid file: {}. Expected first line to contain ivcor (1 or 2) but found an empty line.", path.display())
            )))
        }
    };
    match ivcor {
        1 => unimplemented!("vgrid type 1 not implemented yet"),
        2 => Ok(Box::new(SZ::from_pathbuf_ref(path)?)),
        _ => Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("Error reading vgrid file: {}. Expected first line to contain ivcor (1 or 2) but found {}.", path.display(), ivcor)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sz_from_schism_examples() {
        let path = PathBuf::from("tests/fixtures/vgrid.in.SZ");
        let vgrid = from_pathbuf_ref(&path).unwrap();
        assert_eq!(vgrid.nlevels(), 54);
    }
}
