//! Which DPI values a sensor offers, and the code the mouse expects for each.
//!
//! One table per sensor variant (the status reply's variant byte, `0x95` on the M7 Pro). A table is a list of
//! DPI/50 values; the code sent is the position of the value in the list, plus one on three variants.

use super::DPI_STEP;

/// `(first, last, step)` ranges of DPI/50 values, the code offset, and the DPI sensor name.
struct Spec {
    variant: u8,
    ranges: &'static [(u16, u16, u16)],
    offset: u16,
}

const SPECS: &[Spec] = &[
    Spec { variant: 0x04, ranges: &[(2, 80, 2), (90, 90, 1), (96, 96, 1), (110, 160, 10)], offset: 0 },
    Spec { variant: 0x12, ranges: &[(4, 48, 2), (60, 96, 2)], offset: 1 },
    Spec { variant: 0x25, ranges: &[(4, 100, 2), (110, 110, 1), (120, 120, 1), (128, 128, 1), (140, 200, 10)], offset: 1 },
    Spec { variant: 0x27, ranges: &[(4, 124, 2), (130, 240, 10), (248, 248, 1)], offset: 1 },
    Spec { variant: 0x35, ranges: &[(2, 320, 2)], offset: 0 },
    Spec { variant: 0x60, ranges: &[(2, 240, 2), (480, 480, 1)], offset: 0 },
    Spec { variant: 0x70, ranges: &[(1, 200, 1), (202, 380, 2)], offset: 0 },
    Spec { variant: 0x89, ranges: &[(2, 320, 2)], offset: 0 },
    Spec { variant: 0x95, ranges: &[(1, 520, 1)], offset: 0 },
    Spec { variant: 0x99, ranges: &[(1, 400, 1)], offset: 0 },
];

/// The variant of the M7 Pro (PixArt 3395), used when the mouse has not told us otherwise.
pub const DEFAULT_VARIANT: u8 = 0x95;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DpiTable {
    pub variant: u8,
    /// DPI/50 values in table order.
    values: Vec<u16>,
    offset: u16,
}

impl DpiTable {
    pub fn for_variant(variant: u8) -> Option<DpiTable> {
        let spec = SPECS.iter().find(|s| s.variant == variant)?;
        let values = spec.ranges.iter().flat_map(|&(a, b, step)| (a..=b).step_by(step as usize)).collect();
        Some(DpiTable { variant, values, offset: spec.offset })
    }

    /// The table of the M7 Pro.
    pub fn standard() -> DpiTable {
        DpiTable::for_variant(DEFAULT_VARIANT).expect("the default variant has a table")
    }

    pub fn min_dpi(&self) -> u32 {
        self.values[0] as u32 * DPI_STEP
    }

    pub fn max_dpi(&self) -> u32 {
        *self.values.last().expect("tables are not empty") as u32 * DPI_STEP
    }

    /// The code to send for `dpi`; the value must be in the table.
    pub fn code_for(&self, dpi: u32) -> Result<u16, String> {
        let hit = dpi.is_multiple_of(DPI_STEP).then(|| self.values.iter().position(|&v| v as u32 * DPI_STEP == dpi)).flatten();
        match hit {
            Some(i) => Ok(i as u16 + self.offset),
            None => {
                let near = self.values.iter().map(|&v| v as u32 * DPI_STEP).min_by_key(|v| v.abs_diff(dpi)).unwrap_or(0);
                Err(format!("{dpi} DPI is not available (variant {:#04x}: {}-{} DPI); nearest is {near}", self.variant, self.min_dpi(), self.max_dpi()))
            }
        }
    }

    /// The DPI a code stands for, or `None` for an unknown code.
    pub fn dpi_for_code(&self, code: u16) -> Option<u32> {
        let i = code.checked_sub(self.offset)? as usize;
        self.values.get(i).map(|&v| v as u32 * DPI_STEP)
    }

    /// The DPI of the table entry at `index` (what vendor profile files store).
    pub fn dpi_for_index(&self, index: u16) -> Option<u32> {
        self.values.get(index as usize).map(|&v| v as u32 * DPI_STEP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sizes() {
        for (variant, n) in [(0x04, 48), (0x12, 42), (0x25, 59), (0x27, 74), (0x35, 160), (0x60, 121), (0x70, 290), (0x89, 160), (0x95, 520), (0x99, 400)] {
            let t = DpiTable::for_variant(variant).unwrap();
            assert_eq!(t.values.len(), n, "variant {variant:#x}");
        }
        assert!(DpiTable::for_variant(0xff).is_none());
    }

    #[test]
    fn the_m7_pro_table_is_dpi_over_50_minus_1() {
        let t = DpiTable::standard();
        assert_eq!((t.code_for(800), t.code_for(50), t.code_for(26000)), (Ok(15), Ok(0), Ok(519)));
        assert_eq!(t.dpi_for_code(15), Some(800));
        assert!(t.code_for(825).is_err() && t.code_for(26050).is_err() && t.code_for(0).is_err());
    }

    #[test]
    fn other_variants_have_gaps_and_offsets() {
        let t = DpiTable::for_variant(0x25).unwrap();
        assert_eq!(t.code_for(200), Ok(1), "first value 4 x 50 = 200 DPI gets code 0 + offset 1");
        assert_eq!(t.dpi_for_code(1), Some(200));
        assert!(t.code_for(250).is_err(), "odd multiples of 50 are not in this table");
        assert!(t.code_for(5500).is_ok() && t.code_for(5800).is_err(), "after 5000 only 5500, 6000, 6400 and then steps of 500 exist");
    }
}
