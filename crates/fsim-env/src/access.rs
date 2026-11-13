//! Whether anything can reach a machine. In a real blueprint every machine needs a free tile
//! beside it for an inserter, and a belt must be able to get to that tile from outside the grid
//! (the grid is the whole area a layout may use, so its edge is where inputs and outputs pass).
//!
//! * **Enclosed**: no free tile along any side of the machine. Tiles outside the grid are not
//!   free.
//! * **Cut off**: it has free tiles beside it, but only in pockets of free space sealed off from
//!   the grid edge by other machines.
//! * **Open**: at least [`MIN_FREE_TILES`] free tiles beside it connect (through free tiles) to a
//!   free tile on the grid edge.
//!
//! Enclosed and cut-off machines are disjoint, so each costs once in the reward.

use std::collections::VecDeque;

use crate::report::PlacedView;

/// Free tiles a machine needs beside it to count as reachable. One is enough to place an
/// inserter; raise it to ask for separate access for input and output.
const MIN_FREE_TILES: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    Open,
    CutOff,
    Enclosed,
}

/// The access of every machine, in the order of `placed`.
pub(crate) fn access(placed: &[PlacedView], width: i32, height: i32) -> Vec<Access> {
    let tiles = (width * height) as usize;
    let at = |x: i32, y: i32| (y * width + x) as usize;
    let inside = |x: i32, y: i32| (0..width).contains(&x) && (0..height).contains(&y);

    let mut taken = vec![false; tiles];
    for p in placed {
        for y in p.y.max(0)..(p.y + p.height).min(height) {
            for x in p.x.max(0)..(p.x + p.width).min(width) {
                taken[at(x, y)] = true;
            }
        }
    }

    // free tiles connected to the grid edge: flood fill from the free tiles on the edge
    let mut outside = vec![false; tiles];
    let mut queue = VecDeque::new();
    for y in 0..height {
        for x in 0..width {
            let on_edge = x == 0 || y == 0 || x == width - 1 || y == height - 1;
            if on_edge && !taken[at(x, y)] {
                outside[at(x, y)] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if inside(nx, ny) && !taken[at(nx, ny)] && !outside[at(nx, ny)] {
                outside[at(nx, ny)] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    placed
        .iter()
        .map(|p| {
            let columns = (p.x..p.x + p.width).flat_map(|x| [(x, p.y - 1), (x, p.y + p.height)]);
            let rows = (p.y..p.y + p.height).flat_map(|y| [(p.x - 1, y), (p.x + p.width, y)]);
            let beside: Vec<_> = columns
                .chain(rows)
                .filter(|&(x, y)| inside(x, y) && !taken[at(x, y)])
                .collect();
            let reachable = beside.iter().filter(|&&(x, y)| outside[at(x, y)]).count();
            if beside.len() < MIN_FREE_TILES {
                Access::Enclosed
            } else if reachable < MIN_FREE_TILES {
                Access::CutOff
            } else {
                Access::Open
            }
        })
        .collect()
}

/// The enclosed and the cut-off machines, each over the machines that fit on the grid (not over
/// the machines built, so an extra machine never dilutes them), `0..=1`.
pub(crate) fn ratios(access: &[Access], budget: usize) -> (f64, f64) {
    let share = |wanted: Access| {
        if budget == 0 {
            0.0
        } else {
            (access.iter().filter(|&&a| a == wanted).count() as f64 / budget as f64).min(1.0)
        }
    };
    (share(Access::Enclosed), share(Access::CutOff))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(x: i32, y: i32, size: i32) -> PlacedView {
        PlacedView {
            index: 0,
            machine: "m".into(),
            recipe: "r".into(),
            x,
            y,
            width: size,
            height: size,
        }
    }

    fn at(placed: &[PlacedView], width: i32, height: i32) -> Vec<Access> {
        access(placed, width, height)
    }

    #[test]
    fn a_machine_with_free_sides_is_open_and_the_grid_edge_is_a_wall() {
        assert_eq!(at(&[machine(5, 5, 3)], 15, 15), [Access::Open]);
        // a 3x3 block in the middle: the centre machine has no free tile at all
        let block: Vec<_> = (0..9)
            .map(|i| machine(3 + (i % 3) * 3, 3 + (i / 3) * 3, 3))
            .collect();
        let found = at(&block, 15, 15);
        assert_eq!(found[4], Access::Enclosed);
        assert!(found
            .iter()
            .enumerate()
            .all(|(i, &a)| i == 4 || a == Access::Open));
        // in a corner two neighbours are enough: the grid edge is not free space
        let corner = [machine(0, 0, 3), machine(3, 0, 3), machine(0, 3, 3)];
        assert_eq!(at(&corner, 15, 15)[0], Access::Enclosed);
        // a free tile beside a machine is enough when a neighbour covers only part of the side
        let notch = [
            machine(3, 3, 3),
            machine(0, 3, 3),
            machine(6, 3, 3),
            machine(3, 0, 3),
            machine(3, 7, 3),
        ];
        assert_eq!(at(&notch, 15, 15)[0], Access::Open);
    }

    #[test]
    fn free_tiles_sealed_off_from_the_edge_do_not_count() {
        // 1x1 machines on a 6x6 grid: M at (2,2); its sides (1,2), (3,2), (2,3) are taken and the
        // only free tile beside it, (2,1), is walled in by (1,1), (3,1) and (2,0)
        let sealed: Vec<_> = [(2, 2), (1, 2), (3, 2), (2, 3), (1, 1), (3, 1), (2, 0)]
            .iter()
            .map(|&(x, y)| machine(x, y, 1))
            .collect();
        assert_eq!(at(&sealed, 6, 6)[0], Access::CutOff);
        // open the pocket to the edge by removing (2, 0): now it is reachable
        let opened: Vec<_> = sealed.iter().take(6).cloned().collect();
        assert_eq!(at(&opened, 6, 6)[0], Access::Open);
    }

    #[test]
    fn ratios_count_each_kind_once_against_the_grid_budget() {
        let found = [
            Access::Open,
            Access::Enclosed,
            Access::CutOff,
            Access::CutOff,
        ];
        assert_eq!(ratios(&found, 100), (0.01, 0.02));
        assert_eq!(ratios(&found, 0), (0.0, 0.0));
        assert_eq!(ratios(&[], 100), (0.0, 0.0));
    }
}
