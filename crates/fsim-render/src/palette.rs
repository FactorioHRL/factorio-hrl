pub(crate) type Rgb = (u8, u8, u8);

pub(crate) const BACKGROUND: Rgb = (38, 38, 38);
pub(crate) const GRID: Rgb = (62, 62, 62);

pub(crate) fn color_for(kind: &str) -> Rgb {
    match kind {
        "assembling-machine" => (74, 120, 190),
        "furnace" => (214, 132, 54),
        "lab" => (120, 90, 170),
        "rocket-silo" => (200, 200, 210),
        "beacon" => (96, 190, 190),
        "container" | "logistic-container" => (150, 110, 70),
        "electric-pole" => (120, 200, 90),
        "transport-belt" | "underground-belt" | "splitter" | "loader" | "loader-1x1" => {
            (222, 190, 60)
        }
        "pipe" | "pipe-to-ground" | "storage-tank" | "pump" => (120, 140, 170),
        "inserter" => (210, 90, 70),
        "mining-drill" => (170, 90, 90),
        "boiler" | "generator" | "reactor" | "heat-pipe" | "fusion-reactor" => (190, 70, 70),
        _ => hashed(kind),
    }
}

fn hashed(kind: &str) -> Rgb {
    let hash = kind.bytes().fold(2_166_136_261_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    });
    let channel = |shift: u32| 90 + ((hash >> shift) & 0x5f) as u8;
    (channel(0), channel(8), channel(16))
}

pub(crate) fn darker((r, g, b): Rgb) -> Rgb {
    (r / 2, g / 2, b / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_kinds_are_distinct_and_unknown_are_stable() {
        assert_ne!(color_for("assembling-machine"), color_for("furnace"));
        assert_eq!(color_for("mystery"), color_for("mystery"));
    }
}
