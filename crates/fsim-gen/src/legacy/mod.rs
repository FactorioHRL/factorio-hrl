//! Old template generator (machine bands between lanes). Not random in the sense of
//! "random entities at random places"; it lays out a factory from a template and randomises details.
//! Kept for comparison and as nice material for previewing the renderer.

mod layout;

use fsim_blueprint::model::Design;
use fsim_data::RawData;

use crate::canvas::{self, Canvas};
use crate::error::Result;
use crate::rng::{pick, Rng, SeedableRng};
use crate::Params;
use layout::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Assembler bands between lanes, horizontal.
    Assemblers,
    /// 2×2 furnace bands between lanes, vertical.
    Smelting,
    /// Refineries and chemical plants on top, assembler bands below them.
    Chemical,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Assemblers, Style::Smelting, Style::Chemical];

    /// A style chosen deterministically from `seed`.
    pub fn from_seed(seed: u64) -> Style {
        *pick(&mut Rng::seed_from_u64(seed), &Style::ALL)
    }

    pub fn name(self) -> &'static str {
        match self {
            Style::Assemblers => "assemblers",
            Style::Smelting => "smelting",
            Style::Chemical => "chemical",
        }
    }
}

/// The same `(style, params)` always gives the same design. `params.entities` is ignored.
pub fn generate(data: &RawData, style: Style, params: Params) -> Result<Design> {
    let mut rng = Rng::seed_from_u64(params.seed);
    let (w, h) = (params.width, params.height);
    match style {
        Style::Assemblers => {
            let mut c = Canvas::new(data, w, h);
            let dir = layout::random_horizontal(&mut rng);
            layout::bands(&mut c, &mut rng, 0, h, Kind::Assembler, dir)?;
            Ok(c.into_design())
        }
        Style::Smelting => {
            // laid out horizontally on a canvas with swapped dimensions, then rotated
            let mut c = Canvas::new(data, h, w);
            let dir = layout::random_horizontal(&mut rng);
            layout::bands(&mut c, &mut rng, 0, w, Kind::Furnace, dir)?;
            let mut design = c.into_design();
            canvas::rotate_cw(&mut design, w);
            Ok(design)
        }
        Style::Chemical => {
            let mut c = Canvas::new(data, w, h);
            layout::oil_block(&mut c, &mut rng, 0)?;
            let dir = layout::random_horizontal(&mut rng);
            layout::oil_belt(&mut c, &mut rng, layout::OIL_ROWS - 1, dir);
            layout::bands(
                &mut c,
                &mut rng,
                layout::OIL_ROWS,
                h - layout::OIL_ROWS,
                Kind::Assembler,
                dir.opposite(),
            )?;
            Ok(c.into_design())
        }
    }
}
