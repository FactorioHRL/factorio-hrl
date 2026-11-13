//! Steady state with limited supply: how much of a target item a set of machines can make
//! when the raw inputs arrive at fixed rates. Logistics is ignored (every machine can reach
//! every item), so this is the throughput ceiling of a layout, solved as a small linear program.

use std::collections::BTreeMap;

use fsim_blueprint::model::Design;
use fsim_data::RawData;
use good_lp::{constraint, microlp, variable, Expression, ProblemVariables, Solution, SolverModel};

use crate::{into_flows, net_per_craft, Error, Flow, Result};

/// Items arriving per second from outside (everything else must be made by the machines):
/// `Some(rate)` is a limit, `None` means as much as needed, and an absent item has no supply.
pub type Supply = BTreeMap<String, Option<f64>>;

/// Penalty per craft, so machines that add nothing to the target stay idle instead of running.
const IDLE_PREFERENCE: f64 = 1e-4;

/// All machines of one kind running one recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub machine: String,
    pub recipe: String,
    pub count: usize,
    /// Crafts per second when every machine of the group runs flat out.
    pub max_crafts_per_second: f64,
    /// Crafts per second in the steady state.
    pub crafts_per_second: f64,
}

impl Group {
    /// Share of the group's capacity that is used, in `0..=1`.
    pub fn utilisation(&self) -> f64 {
        if self.max_crafts_per_second > 0.0 {
            (self.crafts_per_second / self.max_crafts_per_second).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SteadyState {
    /// Net production of the target item per second.
    pub target_per_second: f64,
    pub groups: Vec<Group>,
    /// Net balance per item (negative: taken from the supply), see `net_flows` for the order.
    pub flows: Vec<Flow>,
}

/// One kind of machine running one recipe, as the program sees it.
struct Spec {
    net: Vec<(String, f64)>,
    /// Crafts per second of one machine.
    rate: f64,
    /// `Some(n)`: exactly `n` machines. `None`: as many as the machine budget allows.
    count: Option<usize>,
}

/// Best steady state of the machines in `design` for the given `supply`, maximising `target`.
/// With `cap` set, the net production of `target` is limited to it, so machines beyond what the
/// cap needs stay idle (a layout asked for a specific output, not for as much as possible).
pub fn steady_state(
    design: &Design,
    data: &RawData,
    supply: &Supply,
    target: &str,
    cap: Option<f64>,
) -> Result<SteadyState> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for entity in &design.entities {
        if let Some(recipe) = &entity.recipe {
            *counts
                .entry((entity.name.to_string(), recipe.name.to_string()))
                .or_default() += 1;
        }
    }
    let mut groups = Vec::new();
    let mut specs = Vec::new();
    for ((machine, recipe), count) in counts {
        let Some((spec, max)) = spec_for(data, &machine, &recipe, Some(count))? else {
            continue;
        };
        specs.push(spec);
        groups.push(Group {
            machine,
            recipe,
            count,
            max_crafts_per_second: max,
            crafts_per_second: 0.0,
        });
    }
    let solved = solve(&specs, None, supply, target, cap)?;
    for (group, crafts) in groups.iter_mut().zip(&solved.crafts) {
        group.crafts_per_second = *crafts;
    }
    let mut net: BTreeMap<String, f64> = BTreeMap::new();
    for (spec, crafts) in specs.iter().zip(&solved.crafts) {
        for (item, amount) in &spec.net {
            *net.entry(item.clone()).or_default() += amount * crafts;
        }
    }
    Ok(SteadyState {
        target_per_second: solved.target,
        groups,
        flows: into_flows(net),
    })
}

/// Upper bound of the target rate when at most `max_machines` machines may be built, each
/// being any of the `(machine, recipe)` kinds. Counts are relaxed to real numbers, so this is
/// an optimistic ceiling.
pub fn ceiling(
    data: &RawData,
    kinds: &[(String, String)],
    max_machines: usize,
    supply: &Supply,
    target: &str,
) -> Result<f64> {
    let mut specs = Vec::new();
    for (machine, recipe) in kinds {
        if let Some((spec, _)) = spec_for(data, machine, recipe, None)? {
            specs.push(spec);
        }
    }
    Ok(solve(&specs, Some(max_machines), supply, target, None)?.target)
}

/// Returns the program row and the group's maximum crafts per second, or `None` for machines
/// or recipes missing from the data and machines with no crafting speed.
fn spec_for(
    data: &RawData,
    machine: &str,
    recipe: &str,
    count: Option<usize>,
) -> Result<Option<(Spec, f64)>> {
    let speed = data
        .machine(machine)?
        .map(|m| m.crafting_speed)
        .filter(|s| *s > 0.0);
    let (Some(speed), Some(recipe)) = (speed, data.recipe(recipe)?) else {
        return Ok(None);
    };
    let rate = speed / recipe.energy_required;
    let spec = Spec {
        net: net_per_craft(&recipe),
        rate,
        count,
    };
    Ok(Some((spec, rate * count.unwrap_or(1) as f64)))
}

struct Solved {
    /// Crafts per second of each spec.
    crafts: Vec<f64>,
    target: f64,
}

fn solve(
    specs: &[Spec],
    machine_budget: Option<usize>,
    supply: &Supply,
    target: &str,
    cap: Option<f64>,
) -> Result<Solved> {
    let mut vars = ProblemVariables::new();
    let crafts: Vec<_> = specs
        .iter()
        .map(|_| vars.add(variable().min(0.0)))
        .collect();
    let machines: Vec<_> = specs
        .iter()
        .map(|_| vars.add(variable().min(0.0)))
        .collect();

    let mut balance: BTreeMap<&str, Expression> = BTreeMap::new();
    for (spec, x) in specs.iter().zip(&crafts) {
        for (item, amount) in &spec.net {
            *balance.entry(item).or_insert_with(|| Expression::from(0.0)) += *x * *amount;
        }
    }
    let target_net = balance
        .get(target)
        .cloned()
        .unwrap_or_else(|| Expression::from(0.0));
    let total_crafts: Expression = crafts.iter().copied().sum();

    let mut constraints = Vec::new();
    for (item, net) in &balance {
        // an unlimited item (`None`) may be drawn freely, so it gets no constraint
        if let Some(rate) = supply.get(*item).copied().unwrap_or(Some(0.0)) {
            constraints.push(constraint!(net.clone() >= -rate));
        }
    }
    for ((spec, x), n) in specs.iter().zip(&crafts).zip(&machines) {
        match spec.count {
            Some(count) => constraints.push(constraint!(*x <= spec.rate * count as f64)),
            None => constraints.push(constraint!(*x <= *n * spec.rate)),
        }
    }
    if let Some(cap) = cap {
        constraints.push(constraint!(target_net.clone() <= cap));
    }
    if let Some(budget) = machine_budget {
        let total: Expression = machines.iter().copied().sum();
        constraints.push(constraint!(total <= budget as f64));
    }

    let objective = target_net.clone() - total_crafts * IDLE_PREFERENCE;
    let solution = vars
        .maximise(objective)
        .using(microlp)
        .with_all(constraints)
        .solve()
        .map_err(|e| Error::Solver(e.to_string()))?;
    Ok(Solved {
        crafts: crafts.iter().map(|x| solution.value(*x).max(0.0)).collect(),
        target: solution.eval(&target_net).max(0.0),
    })
}

#[cfg(test)]
mod tests {
    use fsim_blueprint::model::{CenterPos, Direction, Entity, Quality, Recipe};

    use super::*;

    /// Cable: 1 copper -> 2 cable in 0.5 s. Circuit: 1 iron + 3 cable -> 1 circuit in 0.5 s.
    /// An assembler running at speed 0.5 therefore makes 1 craft per second.
    fn data() -> RawData {
        RawData::from_slice(
            br#"{
            "assembling-machine": {
                "asm": {"name": "asm", "crafting_speed": 0.5, "crafting_categories": ["crafting"]}
            },
            "recipe": {
                "cable": {"name": "cable", "energy_required": 0.5,
                    "ingredients": [{"name": "copper", "amount": 1}],
                    "results": [{"name": "cable", "amount": 2}]},
                "circuit": {"name": "circuit", "energy_required": 0.5,
                    "ingredients": [{"name": "iron", "amount": 1}, {"name": "cable", "amount": 3}],
                    "results": [{"name": "circuit", "amount": 1}]}
            }
        }"#,
        )
        .unwrap()
    }

    fn design(cables: usize, circuits: usize) -> Design {
        let machine = |recipe: &str, i: usize| Entity {
            name: "asm".into(),
            pos: CenterPos::new(i as i32 * 4, 0),
            direction: Direction::NORTH,
            mirror: false,
            quality: Quality::normal(),
            recipe: Some(Recipe {
                name: recipe.into(),
                quality: Quality::normal(),
            }),
            modules: Vec::new(),
            io: None,
        };
        Design {
            entities: (0..cables)
                .map(|i| machine("cable", i))
                .chain((0..circuits).map(|i| machine("circuit", cables + i)))
                .collect(),
        }
    }

    fn supply(iron: f64, copper: f64) -> Supply {
        Supply::from([
            ("iron".to_owned(), Some(iron)),
            ("copper".to_owned(), Some(copper)),
        ])
    }

    fn circuits(cables: usize, circuits_machines: usize, supply: &Supply) -> SteadyState {
        steady_state(
            &design(cables, circuits_machines),
            &data(),
            supply,
            "circuit",
            None,
        )
        .unwrap()
    }

    #[test]
    fn ratio_of_15_cable_to_10_circuit_machines_uses_everything() {
        let state = circuits(15, 10, &supply(10.0, 15.0));
        assert!((state.target_per_second - 10.0).abs() < 1e-6);
        assert!(state
            .groups
            .iter()
            .all(|g| (g.utilisation() - 1.0).abs() < 1e-6));
    }

    #[test]
    fn missing_cable_machines_limit_the_output() {
        // 6 cable machines make 12 cable/s = 4 circuit/s, whatever the circuit machines count
        let state = circuits(6, 10, &supply(10.0, 15.0));
        assert!((state.target_per_second - 4.0).abs() < 1e-6);
    }

    #[test]
    fn extra_machines_stay_idle_when_supply_is_the_limit() {
        // supply allows 5 circuit/s; 10 circuit machines are twice what is needed
        let state = circuits(15, 10, &supply(5.0, 7.5));
        assert!((state.target_per_second - 5.0).abs() < 1e-6);
        let circuit = state.groups.iter().find(|g| g.recipe == "circuit").unwrap();
        assert!((circuit.utilisation() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_cap_keeps_the_output_down_and_the_extra_machines_idle() {
        let mut unlimited = supply(0.0, 0.0);
        unlimited.insert("iron".into(), None);
        unlimited.insert("copper".into(), None);
        let state =
            steady_state(&design(15, 10), &data(), &unlimited, "circuit", Some(4.0)).unwrap();
        assert!((state.target_per_second - 4.0).abs() < 1e-6);
        let circuit = state.groups.iter().find(|g| g.recipe == "circuit").unwrap();
        assert!((circuit.utilisation() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn unlimited_supply_reports_what_the_layout_would_consume() {
        let mut unlimited = supply(0.0, 0.0);
        unlimited.insert("iron".into(), None);
        unlimited.insert("copper".into(), None);
        let state = circuits(15, 10, &unlimited);
        assert!((state.target_per_second - 10.0).abs() < 1e-6);
        let rate = |name: &str| {
            state
                .flows
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.per_second)
        };
        assert!((rate("iron").unwrap() + 10.0).abs() < 1e-6);
        assert!((rate("copper").unwrap() + 15.0).abs() < 1e-6);
    }

    #[test]
    fn without_a_producer_of_an_intermediate_nothing_is_made() {
        assert_eq!(circuits(0, 5, &supply(10.0, 15.0)).target_per_second, 0.0);
    }

    #[test]
    fn ceiling_with_a_machine_budget_splits_it_optimally() {
        let kinds = [
            ("asm".to_owned(), "cable".to_owned()),
            ("asm".to_owned(), "circuit".to_owned()),
        ];
        // c cable + k circuit machines: circuits = min(k, 2c/3, supplies). 25 machines -> 10
        let best = ceiling(&data(), &kinds, 25, &supply(10.0, 15.0), "circuit").unwrap();
        assert!((best - 10.0).abs() < 1e-6);
        // 10 machines: c + k = 10, k = 2c/3 -> c = 6, k = 4
        let best = ceiling(&data(), &kinds, 10, &supply(10.0, 15.0), "circuit").unwrap();
        assert!((best - 4.0).abs() < 1e-6);
    }
}
