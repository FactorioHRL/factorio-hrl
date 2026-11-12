#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Direction(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionScale {
    Eight,
    Sixteen,
}

impl Direction {
    pub const COUNT: u8 = 16;
    pub const NORTH: Direction = Direction(0);
    pub const EAST: Direction = Direction(4);
    pub const SOUTH: Direction = Direction(8);
    pub const WEST: Direction = Direction(12);

    pub fn new(value: u8) -> Option<Self> {
        (value < Self::COUNT).then_some(Direction(value))
    }

    pub fn from_blueprint(raw: u8, scale: DirectionScale) -> Option<Self> {
        match scale {
            DirectionScale::Sixteen => Self::new(raw),
            DirectionScale::Eight => (raw < 8).then(|| Direction(raw * 2)),
        }
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn is_cardinal(self) -> bool {
        self.0 % 4 == 0
    }

    pub fn rotate_cw(self, steps: u8) -> Self {
        Direction((self.0 + steps % Self::COUNT) % Self::COUNT)
    }

    pub fn opposite(self) -> Self {
        self.rotate_cw(Self::COUNT / 2)
    }
}

impl Default for Direction {
    fn default() -> Self {
        Self::NORTH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_are_normalized() {
        let east_1_1 = Direction::from_blueprint(2, DirectionScale::Eight);
        let east_2_0 = Direction::from_blueprint(4, DirectionScale::Sixteen);
        assert_eq!(east_1_1, Some(Direction::EAST));
        assert_eq!(east_1_1, east_2_0);
        assert_eq!(Direction::from_blueprint(8, DirectionScale::Eight), None);
        assert_eq!(Direction::from_blueprint(16, DirectionScale::Sixteen), None);
    }

    #[test]
    fn rotation() {
        assert_eq!(Direction::WEST.rotate_cw(4), Direction::NORTH);
        assert_eq!(Direction::NORTH.opposite(), Direction::SOUTH);
        assert!(Direction::EAST.is_cardinal());
        assert!(!Direction::new(2).unwrap().is_cardinal());
    }
}
