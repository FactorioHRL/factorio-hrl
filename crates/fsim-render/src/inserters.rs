use fsim_blueprint::model::Direction;
use serde_json::Value;

use crate::sprite::{named_layers, pair, SpriteLayer};

pub(crate) const INSERTER: &str = "inserter";

const DEGREES_PER_STEP: f64 = 360.0 / 16.0;
const SOUTH_STEPS: f64 = 8.0;
pub(crate) const CLAW_TIP_OVERSHOOT: f64 = 0.15;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Arm {
    pub angle_deg: f32,
    pub reach: f64,
    pub base: Option<SpriteLayer>,
    pub hand: Option<SpriteLayer>,
}

pub(crate) fn platform(prototype: &Value, direction: Direction) -> Vec<SpriteLayer> {
    named_layers(prototype, "platform_picture", direction)
}

pub(crate) fn arm(prototype: &Value, direction: Direction) -> Option<Arm> {
    let (x, y) = prototype.get("pickup_position").and_then(pair)?;
    let from_south = (direction.index() as f64 - SOUTH_STEPS) * DEGREES_PER_STEP;
    let first = |key| {
        named_layers(prototype, key, Direction::NORTH)
            .into_iter()
            .next()
    };
    Some(Arm {
        angle_deg: (x.atan2(-y).to_degrees() + from_south) as f32,
        reach: x.hypot(y),
        base: first("hand_base_picture"),
        hand: first("hand_open_picture"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn inserter(pickup: Value) -> Value {
        json!({
            "pickup_position": pickup,
            "platform_picture": {"sheet": {"filename": "p.png", "width": 105, "height": 79}},
            "hand_base_picture": {"filename": "b.png", "width": 32, "height": 136, "scale": 0.25},
            "hand_open_picture": {"filename": "h.png", "width": 72, "height": 164, "scale": 0.25}
        })
    }

    #[test]
    fn prototype_vectors_describe_a_south_facing_inserter() {
        let proto = inserter(json!([0, -1]));
        let angle = |direction| arm(&proto, direction).unwrap().angle_deg;
        assert_eq!(angle(Direction::SOUTH), 0.0);
        assert_eq!(angle(Direction::WEST), 90.0);
        assert_eq!(angle(Direction::NORTH), -180.0);
        assert_eq!(angle(Direction::EAST), -90.0);
    }

    #[test]
    fn reach_is_the_pickup_distance() {
        assert_eq!(
            arm(&inserter(json!([0, -1])), Direction::SOUTH)
                .unwrap()
                .reach,
            1.0
        );
        assert_eq!(
            arm(&inserter(json!([0, -2])), Direction::SOUTH)
                .unwrap()
                .reach,
            2.0
        );
    }

    #[test]
    fn hand_parts_and_platform_come_from_the_prototype() {
        let proto = inserter(json!([0, -1]));
        let parts = arm(&proto, Direction::SOUTH).unwrap();
        assert_eq!(parts.base.unwrap().filename, "b.png");
        assert_eq!(parts.hand.unwrap().filename, "h.png");
        assert_eq!(platform(&proto, Direction::EAST)[0].x, 105);
    }

    #[test]
    fn prototype_without_a_pickup_position_has_no_arm() {
        assert!(arm(&json!({}), Direction::SOUTH).is_none());
    }
}
