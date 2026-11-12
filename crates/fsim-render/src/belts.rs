use fsim_blueprint::model::IoType;

use crate::grid::{step, TileGrid};
use crate::layout::{quarter_turns, Placed};

const BELT: &str = "transport-belt";
pub(crate) const SPLITTER: &str = "splitter";
pub(crate) const UNDERGROUND: &str = "underground-belt";
pub(crate) const LOADER: &str = "loader";
const FEEDING_KINDS: [&str; 3] = ["transport-belt", "underground-belt", "splitter"];

const STRAIGHT: [u8; 4] = [3, 1, 4, 2];
const STARTING: [u8; 4] = [17, 19, 13, 15];
const CURVES: [(u8, u8, u8); 8] = [
    (1, 0, 5),
    (0, 1, 6),
    (3, 0, 7),
    (0, 3, 8),
    (2, 1, 9),
    (1, 2, 10),
    (2, 3, 11),
    (3, 2, 12),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BeltShape {
    Straight { start: bool },
    Curve { from: u8 },
}

impl BeltShape {
    pub fn variants(self, facing: u8) -> Vec<u8> {
        let straight = STRAIGHT[usize::from(facing)];
        match self {
            BeltShape::Straight { start: false } => vec![straight],
            BeltShape::Straight { start: true } => vec![straight, STARTING[usize::from(facing)]],
            BeltShape::Curve { from } => CURVES
                .iter()
                .find(|&&(f, t, _)| f == from && t == facing)
                .map_or_else(|| vec![straight], |&(_, _, index)| vec![index]),
        }
    }
}

pub(crate) fn shapes(placed: &[Placed]) -> Vec<Option<BeltShape>> {
    let grid = TileGrid::new(placed);
    placed
        .iter()
        .map(|p| (p.kind == BELT).then(|| shape(p, placed, &grid)))
        .collect()
}

fn shape(belt: &Placed, placed: &[Placed], grid: &TileGrid) -> BeltShape {
    let facing = quarter_turns(belt.direction);
    let (x, y) = (
        (belt.cx - 0.5).round() as i32,
        (belt.cy - 0.5).round() as i32,
    );

    let fed_from = |side: u8| {
        let (dx, dy) = step(side);
        grid.at(x + dx, y + dy)
            .map(|i| &placed[i])
            .is_some_and(|n| {
                FEEDING_KINDS.contains(&n.kind.as_str())
                    && n.io != Some(IoType::Input)
                    && quarter_turns(n.direction) == (side + 2) % 4
            })
    };
    let (back, left, right) = (
        fed_from((facing + 2) % 4),
        fed_from((facing + 3) % 4),
        fed_from((facing + 1) % 4),
    );

    match (back, left, right) {
        (false, true, false) => BeltShape::Curve {
            from: (facing + 3) % 4,
        },
        (false, false, true) => BeltShape::Curve {
            from: (facing + 1) % 4,
        },
        (false, false, false) => BeltShape::Straight { start: true },
        _ => BeltShape::Straight { start: false },
    }
}

#[cfg(test)]
mod tests {
    use fsim_blueprint::model::Direction;

    use super::*;

    const N: u8 = 0;
    const E: u8 = 1;
    const S: u8 = 2;
    const W: u8 = 3;

    fn piece(kind: &str, x: i32, y: i32, facing: u8) -> Placed {
        Placed {
            name: kind.into(),
            kind: kind.into(),
            direction: Direction::new(facing * 4).unwrap(),
            io: None,
            cx: f64::from(x) + 0.5,
            cy: f64::from(y) + 0.5,
            width: 1.0,
            height: 1.0,
        }
    }

    fn belt(x: i32, y: i32, facing: u8) -> Placed {
        piece(BELT, x, y, facing)
    }

    #[test]
    fn underground_input_does_not_feed_the_tile_in_front() {
        let mut input = piece("underground-belt", 0, 1, N);
        input.io = Some(IoType::Input);
        let mut output = input.clone();
        output.io = Some(IoType::Output);
        assert_eq!(
            shapes(&[input, belt(0, 0, N)])[1],
            Some(BeltShape::Straight { start: true })
        );
        assert_eq!(
            shapes(&[output, belt(0, 0, N)])[1],
            Some(BeltShape::Straight { start: false })
        );
    }

    #[test]
    fn lone_belt_is_a_straight_start() {
        assert_eq!(
            shapes(&[belt(0, 0, N)]),
            [Some(BeltShape::Straight { start: true })]
        );
    }

    #[test]
    fn belt_fed_from_behind_is_a_plain_straight() {
        let s = shapes(&[belt(0, 1, N), belt(0, 0, N)]);
        assert_eq!(s[0], Some(BeltShape::Straight { start: true }));
        assert_eq!(s[1], Some(BeltShape::Straight { start: false }));
    }

    #[test]
    fn single_side_feeder_makes_a_curve_toward_that_side() {
        let from_west = shapes(&[belt(0, 0, E), belt(1, 0, N)]);
        assert_eq!(from_west[1], Some(BeltShape::Curve { from: W }));
        let from_east = shapes(&[belt(2, 0, W), belt(1, 0, N)]);
        assert_eq!(from_east[1], Some(BeltShape::Curve { from: E }));
        let facing_south = shapes(&[belt(0, 0, E), belt(1, 0, S)]);
        assert_eq!(facing_south[1], Some(BeltShape::Curve { from: W }));
        let facing_east = shapes(&[belt(1, -1, S), belt(1, 0, E)]);
        assert_eq!(facing_east[1], Some(BeltShape::Curve { from: N }));
    }

    #[test]
    fn side_loading_onto_a_fed_belt_stays_straight() {
        let s = shapes(&[belt(1, 1, N), belt(0, 0, E), belt(1, 0, N)]);
        assert_eq!(s[2], Some(BeltShape::Straight { start: false }));
    }

    #[test]
    fn feeders_on_both_sides_stay_straight() {
        let s = shapes(&[belt(0, 0, E), belt(2, 0, W), belt(1, 0, N)]);
        assert_eq!(s[2], Some(BeltShape::Straight { start: false }));
    }

    #[test]
    fn belts_facing_away_do_not_feed() {
        let s = shapes(&[belt(0, 0, W), belt(1, 0, N)]);
        assert_eq!(s[1], Some(BeltShape::Straight { start: true }));
        let s = shapes(&[belt(1, -1, N), belt(1, 0, N)]);
        assert_eq!(s[1], Some(BeltShape::Straight { start: true }));
    }

    #[test]
    fn splitters_and_undergrounds_feed_but_other_entities_do_not() {
        for kind in ["splitter", "underground-belt"] {
            let s = shapes(&[piece(kind, 0, 0, E), belt(1, 0, N)]);
            assert_eq!(s[1], Some(BeltShape::Curve { from: W }), "{kind}");
        }
        let s = shapes(&[piece("inserter", 0, 0, E), belt(1, 0, N)]);
        assert_eq!(s[1], Some(BeltShape::Straight { start: true }));
        assert_eq!(s[0], None);
    }

    #[test]
    fn wide_feeders_are_found_through_every_tile_they_cover() {
        let mut splitter = piece("splitter", 0, 0, E);
        splitter.cx = 0.5;
        splitter.cy = 1.0;
        splitter.height = 2.0;
        let s = shapes(&[splitter, belt(1, 1, N)]);
        assert_eq!(s[1], Some(BeltShape::Curve { from: W }));
    }

    #[test]
    fn variant_indices_match_the_base_game_sheet() {
        let straight = |facing| BeltShape::Straight { start: false }.variants(facing);
        assert_eq!([N, E, S, W].map(straight), [[3], [1], [4], [2]]);
        let start = |facing| BeltShape::Straight { start: true }.variants(facing)[1];
        assert_eq!([N, E, S, W].map(start), [17, 19, 13, 15]);

        let curve = |from, to| BeltShape::Curve { from }.variants(to);
        assert_eq!(curve(E, N), [5]);
        assert_eq!(curve(N, E), [6]);
        assert_eq!(curve(W, N), [7]);
        assert_eq!(curve(N, W), [8]);
        assert_eq!(curve(S, E), [9]);
        assert_eq!(curve(E, S), [10]);
        assert_eq!(curve(S, W), [11]);
        assert_eq!(curve(W, S), [12]);
    }
}
