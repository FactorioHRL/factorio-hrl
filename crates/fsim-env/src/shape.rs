//! Shape of a layout: how machines doing the same thing sit relative to each other. Three numbers,
//! each in `0..=1`, which the Python side weighs into a reward (`scoring.py`). (Whether machines
//! can be reached at all is `access.rs`.)
//!
//! Terms: *twins* are machines with the same machine name and recipe, so the same footprint.
//!
//! * **adjacency**: twins touching along a *whole* side (same row or column, no offset) form a
//!   *cluster*. A cluster that is one straight run (all in one row, or all in one column) of at
//!   least two machines earns `machines - 1` contacts; a cluster with a bend, a branch or a
//!   second row (a T, an L, a block, a snake) earns nothing. The ratio is the contacts over the
//!   machines of kinds built at least twice (a pair rates 0.5, a long row almost 1).
//! * **line**: a straight cluster of at least [`MIN_LINE`] machines. A cluster with a bend, a
//!   branch or a second row is no line, and none of its machines is, so a line cannot be grown
//!   into a maze and still collect the reward. Each machine of a kind built at least [`MIN_LINE`]
//!   times rates `min(length of its line, LINE_CAP) / LINE_CAP` (0 outside a line); the ratio is
//!   the mean.
//! * **stray**: how scattered a kind is. Its machines form *units*: every line is one unit (its
//!   machines count as a single machine, so building a line is never punished) and every
//!   machine outside a line is a unit of its own (a block or a T is just touching singles). The units are joined by the shortest possible
//!   links (a minimum spanning tree, link = tiles between footprints); each link counts its
//!   length capped at half the larger grid side and divided by it. A kind rates
//!   `sum of its links / (machines - 1)`, so one unit alone is 0 and all machines apart and far
//!   from each other is 1. Two far lines of five rate 1/9, a line of three with seven machines
//!   strewn about 7/9, and a compact cluster of singles 0 with or without a line. Only the
//!   shortest links count, so a new line or cluster started away from the rest costs one link,
//!   not one per machine. The term is the mean over the machines of kinds built at least twice.

use std::collections::BTreeMap;

use crate::report::{far_tiles, PlacedView};

/// Shortest run of twins that counts as a line.
const MIN_LINE: usize = 3;
/// Line length that earns the full line rating.
const LINE_CAP: usize = 6;

/// The three numbers of the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Shape {
    pub adjacency: f64,
    pub line: f64,
    pub stray: f64,
}

type Kind<'a> = (&'a str, &'a str);

/// Twins touching along a whole side (twins share one footprint, so one `width`/`height`).
fn full_contact(a: &PlacedView, b: &PlacedView) -> bool {
    (a.y == b.y && (a.x - b.x).abs() == a.width) || (a.x == b.x && (a.y - b.y).abs() == a.height)
}

/// Whether a cluster (positions in `members`, joined by full contact) is a single row or column.
fn straight(placed: &[PlacedView], members: &[usize], cluster: &[usize]) -> bool {
    let first = &placed[members[cluster[0]]];
    cluster.iter().all(|&at| placed[members[at]].y == first.y)
        || cluster.iter().all(|&at| placed[members[at]].x == first.x)
}

/// Every straight cluster of at least two twins of a kind, as positions in `members`: a whole
/// cluster (machines joined by full contact) that is one row or one column. They never share a
/// machine; the runs of at least [`MIN_LINE`] are the lines.
fn straight_clusters(placed: &[PlacedView], members: &[usize]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; members.len()];
    let mut found = Vec::new();
    for start in 0..members.len() {
        if seen[start] {
            continue;
        }
        // the cluster of `start`: everything reachable through full contact
        let mut cluster = Vec::new();
        let mut pending = vec![start];
        seen[start] = true;
        while let Some(at) = pending.pop() {
            cluster.push(at);
            for other in 0..members.len() {
                if !seen[other] && full_contact(&placed[members[at]], &placed[members[other]]) {
                    seen[other] = true;
                    pending.push(other);
                }
            }
        }
        if cluster.len() >= 2 && straight(placed, members, &cluster) {
            found.push(cluster);
        }
    }
    found
}

/// Every line is one unit; every machine outside a line is a unit of its own. Positions refer to
/// the kind's `members`.
fn units(count: usize, lines: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut in_line = vec![false; count];
    for &at in lines.iter().flatten() {
        in_line[at] = true;
    }
    let singles = (0..count).filter(|&at| !in_line[at]).map(|at| vec![at]);
    lines.iter().cloned().chain(singles).collect()
}

/// See the module docs ("stray"): `0` for one unit, up to `1` for everything apart and far.
fn scatter(placed: &[PlacedView], members: &[usize], units: &[Vec<usize>], far: f64) -> f64 {
    let between = |a: &[usize], b: &[usize]| {
        a.iter()
            .flat_map(|&i| {
                b.iter()
                    .map(move |&j| placed[members[i]].gap(&placed[members[j]]))
            })
            .min()
            .unwrap_or(0)
    };
    // Prim's algorithm: grow a tree from the first unit, always by the shortest link
    let mut nearest: Vec<i32> = units.iter().map(|u| between(&units[0], u)).collect();
    let mut inside = vec![false; units.len()];
    inside[0] = true;
    let mut links = 0.0;
    for _ in 1..units.len() {
        let next = (0..units.len())
            .filter(|&u| !inside[u])
            .min_by_key(|&u| nearest[u])
            .expect("a unit is left outside the tree");
        links += (f64::from(nearest[next]) / far).min(1.0);
        inside[next] = true;
        for u in (0..units.len()).filter(|&u| !inside[u]) {
            nearest[u] = nearest[u].min(between(&units[next], &units[u]));
        }
    }
    links / (members.len() - 1) as f64
}

pub(crate) fn shape(placed: &[PlacedView], width: i32, height: i32) -> Shape {
    let mut kinds: BTreeMap<Kind, Vec<usize>> = BTreeMap::new();
    for (i, p) in placed.iter().enumerate() {
        kinds.entry((&p.machine, &p.recipe)).or_default().push(i);
    }
    let far = far_tiles(width, height);
    let (mut adjacency, mut adjacency_of) = (0.0, 0usize);
    let (mut line, mut line_of) = (0.0, 0usize);
    let (mut stray, mut stray_of) = (0.0, 0usize);
    for members in kinds.values().filter(|m| m.len() >= 2) {
        let straight = straight_clusters(placed, members);
        // n twins in a straight run touch n - 1 times
        adjacency += straight.iter().map(|c| (c.len() - 1) as f64).sum::<f64>();
        adjacency_of += members.len();
        let found: Vec<Vec<usize>> = straight
            .into_iter()
            .filter(|c| c.len() >= MIN_LINE)
            .collect();
        if members.len() >= MIN_LINE {
            line += found
                .iter()
                .map(|l| l.len() as f64 * l.len().min(LINE_CAP) as f64 / LINE_CAP as f64)
                .sum::<f64>();
            line_of += members.len();
        }
        let units = units(members.len(), &found);
        stray += scatter(placed, members, &units, far) * members.len() as f64;
        stray_of += members.len();
    }
    let mean = |sum: f64, count: usize| if count == 0 { 0.0 } else { sum / count as f64 };
    Shape {
        adjacency: mean(adjacency, adjacency_of),
        line: mean(line, line_of),
        stray: mean(stray, stray_of),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(recipe: &str, x: i32, y: i32) -> PlacedView {
        PlacedView {
            index: 0,
            machine: "asm".into(),
            recipe: recipe.into(),
            x,
            y,
            width: 3,
            height: 3,
        }
    }

    fn of(placed: &[PlacedView]) -> Shape {
        shape(placed, 30, 30)
    }

    #[test]
    fn only_whole_side_contact_counts_for_adjacency() {
        assert_eq!(of(&[view("a", 0, 0), view("a", 3, 0)]).adjacency, 0.5);
        let offset = [view("a", 0, 0), view("a", 3, 1)];
        assert_eq!(
            of(&offset).adjacency,
            0.0,
            "touching by half a side is not enough"
        );
        let diagonal = [view("a", 0, 0), view("a", 3, 3)];
        assert_eq!(of(&diagonal).adjacency, 0.0);
        let block = [
            view("a", 0, 0),
            view("a", 3, 0),
            view("a", 0, 3),
            view("a", 3, 3),
        ];
        assert_eq!(of(&block).adjacency, 0.0, "a block is no straight run");
        let row = [view("a", 0, 0), view("a", 3, 0), view("a", 6, 0)];
        assert!(
            (of(&row).adjacency - 2.0 / 3.0).abs() < 1e-12,
            "two contacts over three machines"
        );
        let mut snake = row.to_vec();
        snake.extend([view("a", 6, 3), view("a", 6, 6)]); // a row that turns into a column
        assert_eq!(
            of(&snake).adjacency,
            0.0,
            "a bend takes the reward away from the whole cluster"
        );
    }

    #[test]
    fn lines_need_three_twins_in_a_row_or_column_and_longer_rates_higher() {
        let pair = [view("a", 0, 0), view("a", 3, 0)];
        assert_eq!(of(&pair).line, 0.0);
        let three = [view("a", 0, 0), view("a", 3, 0), view("a", 6, 0)];
        assert_eq!(of(&three).line, 0.5);
        let column: Vec<_> = (0..3).map(|i| view("a", 0, i * 3)).collect();
        assert_eq!(of(&column).line, 0.5, "columns count too");
        let six: Vec<_> = (0..6).map(|i| view("a", i * 3, 0)).collect();
        assert_eq!(of(&six).line, 1.0);
        let gapped = [
            view("a", 0, 0),
            view("a", 3, 0),
            view("a", 7, 0),
            view("a", 10, 0),
        ];
        assert_eq!(of(&gapped).line, 0.0, "a gap breaks the line");
    }

    fn row_of(count: i32, x: i32, y: i32) -> Vec<PlacedView> {
        (0..count).map(|i| view("a", x + i * 3, y)).collect()
    }

    #[test]
    fn a_line_counts_as_one_unit_so_far_lines_cost_little_and_strewn_machines_a_lot() {
        // grid 30: links count in full from 15 tiles (half the larger side)
        let mut one_far = row_of(3, 0, 0);
        one_far.push(view("a", 24, 24));
        assert!(
            (of(&one_far).stray - 1.0 / 3.0).abs() < 1e-12,
            "2 units, 4 machines"
        );

        let mut two_lines = row_of(5, 0, 24);
        two_lines.extend(row_of(5, 0, 0));
        assert!(
            (of(&two_lines).stray - 1.0 / 9.0).abs() < 1e-12,
            "two far lines of five: small"
        );

        let singles: Vec<_> = [(0, 0), (24, 0), (0, 24), (24, 24)]
            .iter()
            .map(|&(x, y)| view("a", x, y))
            .collect();
        assert_eq!(
            of(&singles).stray,
            1.0,
            "everything apart: the maximum, without any line"
        );

        // grid 12 (full links from 6 tiles): a line of three, seven singles 6 tiles from each other
        let mut strewn = row_of(3, 0, 0);
        strewn.extend((0..7).map(|i| view("a", 24, i * 9)));
        assert!((shape(&strewn, 12, 12).stray - 7.0 / 9.0).abs() < 1e-12);
    }

    #[test]
    fn starting_a_line_away_from_a_cluster_costs_one_link_not_one_per_machine() {
        let mut cluster: Vec<_> = (0..4)
            .map(|i| view("a", (i % 2) * 3, (i / 2) * 3))
            .collect();
        assert_eq!(of(&cluster).stray, 0.0);
        cluster.extend(row_of(3, 0, 24)); // a new line, far from the 2x2 cluster
        assert!(
            (of(&cluster).stray - 1.0 / 6.0).abs() < 1e-12,
            "one far link among 7 machines"
        );
    }

    #[test]
    fn near_machines_cost_nothing_with_or_without_a_line() {
        let block = [
            view("a", 0, 0),
            view("a", 3, 0),
            view("a", 0, 3),
            view("a", 3, 3),
        ];
        assert_eq!(of(&block).stray, 0.0, "a compact cluster of singles");
        let mut by_the_line = row_of(3, 0, 0);
        by_the_line.push(view("a", 9, 1)); // beside the line end, offset: not in the line, no gap
        assert_eq!(of(&by_the_line).stray, 0.0);
        assert_eq!(of(&row_of(6, 0, 0)).stray, 0.0, "one unit alone");
    }

    #[test]
    fn a_bend_a_branch_or_a_second_row_takes_the_line_away() {
        let row = |n: i32| -> Vec<PlacedView> { row_of(n, 3, 3) };
        assert_eq!(of(&row(3)).line, 0.5, "a clean row is a line");
        // a T: a row of three with a column through its middle
        let mut t = row(3);
        t.extend([view("a", 6, 0), view("a", 6, 6)]);
        assert_eq!(of(&t).line, 0.0, "a T is no line, in neither direction");
        // an L: a row of four turning at its end
        let mut l = row(4);
        l.push(view("a", 12, 6));
        assert_eq!(
            of(&l).line,
            0.0,
            "one machine at a corner spoils the whole cluster"
        );
        // a domino of two rows of three, and a 3x3 block
        let mut domino = row_of(3, 0, 0);
        domino.extend(row_of(3, 0, 3));
        assert_eq!(of(&domino).line, 0.0);
        let block: Vec<_> = (0..9)
            .map(|i| view("a", (i % 3) * 3, (i / 3) * 3))
            .collect();
        assert_eq!(of(&block).line, 0.0);
        // two rows with a gap between them are two lines
        let mut apart = row_of(3, 0, 0);
        apart.extend(row_of(3, 0, 4));
        assert_eq!(of(&apart).line, 0.5);
        // a twin touching the side of a row only by half is not full contact: the line stays
        let mut offset = row_of(3, 0, 0);
        offset.push(view("a", 4, 3));
        assert!((of(&offset).line - 3.0 * 0.5 / 4.0).abs() < 1e-12);
    }

    #[test]
    fn a_block_is_a_cluster_of_touching_singles_for_the_scatter() {
        let block: Vec<_> = (0..9)
            .map(|i| view("a", (i % 3) * 3, (i / 3) * 3))
            .collect();
        let mut with_far = block.clone();
        with_far.push(view("a", 24, 24));
        assert!(
            (of(&with_far).stray - 1.0 / 9.0).abs() < 1e-12,
            "one far link; the block itself costs nothing"
        );
    }

    #[test]
    fn kinds_are_rated_apart_and_empty_layouts_are_neutral() {
        assert_eq!(of(&[]), Shape::default());
        let mixed = [view("a", 0, 0), view("b", 3, 0), view("a", 9, 9)];
        let shape = of(&mixed);
        assert_eq!((shape.adjacency, shape.line), (0.0, 0.0));
        assert!(
            (shape.stray - 0.8).abs() < 1e-12,
            "only the two a machines are rated: 12 tiles apart"
        );
    }
}
