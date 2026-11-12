use crate::fluids::Connections;
use crate::layout::Placed;

const PIPE: &str = "pipe";

const PICTURES: [&str; 16] = [
    "straight_vertical_single",
    "ending_up",
    "ending_right",
    "corner_up_right",
    "ending_down",
    "straight_vertical",
    "corner_down_right",
    "t_right",
    "ending_left",
    "corner_up_left",
    "straight_horizontal",
    "t_up",
    "corner_down_left",
    "t_left",
    "t_down",
    "cross",
];

pub(crate) fn picture_name(mask: u8) -> &'static str {
    PICTURES[usize::from(mask & 0b1111)]
}

pub(crate) fn pictures(placed: &[Placed], connections: &Connections) -> Vec<Option<&'static str>> {
    placed
        .iter()
        .map(|p| {
            (p.kind == PIPE).then(|| {
                let (x, y) = ((p.cx - 0.5).floor() as i32, (p.cy - 0.5).floor() as i32);
                let mask = (0..4)
                    .filter(|&side| connections.faces(x, y, side))
                    .fold(0, |mask, side| mask | 1 << side);
                picture_name(mask)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_bits_are_north_east_south_west() {
        let name = |sides: &[u8]| picture_name(sides.iter().fold(0, |m, s| m | 1 << s));
        assert_eq!(name(&[]), "straight_vertical_single");
        assert_eq!(name(&[0]), "ending_up");
        assert_eq!(name(&[1]), "ending_right");
        assert_eq!(name(&[2]), "ending_down");
        assert_eq!(name(&[3]), "ending_left");
        assert_eq!(name(&[0, 2]), "straight_vertical");
        assert_eq!(name(&[1, 3]), "straight_horizontal");
        assert_eq!(name(&[0, 1]), "corner_up_right");
        assert_eq!(name(&[0, 3]), "corner_up_left");
        assert_eq!(name(&[2, 1]), "corner_down_right");
        assert_eq!(name(&[2, 3]), "corner_down_left");
        assert_eq!(name(&[1, 3, 0]), "t_up");
        assert_eq!(name(&[1, 3, 2]), "t_down");
        assert_eq!(name(&[0, 2, 1]), "t_right");
        assert_eq!(name(&[0, 2, 3]), "t_left");
        assert_eq!(name(&[0, 1, 2, 3]), "cross");
    }

    #[test]
    fn every_mask_has_a_distinct_picture() {
        let mut names: Vec<_> = (0..16).map(picture_name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 16);
    }
}
