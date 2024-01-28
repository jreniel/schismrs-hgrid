# schismrs-hgrid

Basic example.

```Rust
use schismrs_hgrid::Hgrid;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(let path = PathBuf::from("/path/to/my/mesh");;
    let hgrid = Hgrid::try_from(&path)?
    let depths = hgrid.depths()?
    dbg!(depths);
    Ok(())
}
```

No plotting capabilities yet.

### License

`SPDX-License-Identifier: LicenseRef-schismrs-license`
