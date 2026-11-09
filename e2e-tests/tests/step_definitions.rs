#[path = "../src/steps/mod.rs"]
mod steps;
#[path = "../src/utils.rs"]
mod utils;
#[path = "../src/world.rs"]
mod world;

use cucumber::{parser::Basic, Parser, World as _};

#[test]
fn every_feature_step_has_a_registered_handler() {
    let definitions = world::MyWorld::collection();
    let features = Basic::new()
        .parse(
            concat!(env!("CARGO_MANIFEST_DIR"), "/features"),
            Default::default(),
        )
        .into_inner();
    let mut checked_steps = 0;

    for feature in features {
        let feature = feature.expect("Invalid feature file");
        let scenarios = feature
            .scenarios
            .iter()
            .chain(feature.rules.iter().flat_map(|rule| &rule.scenarios));
        let backgrounds = feature.background.iter().chain(
            feature
                .rules
                .iter()
                .filter_map(|rule| rule.background.as_ref()),
        );

        for step in backgrounds
            .flat_map(|background| &background.steps)
            .chain(scenarios.flat_map(|scenario| &scenario.steps))
        {
            assert!(
                definitions
                    .find(step)
                    .expect("Ambiguous step definition")
                    .is_some(),
                "Undefined step in {:?}:{}: {}",
                feature.path,
                step.position.line,
                step.value,
            );
            checked_steps += 1;
        }
    }

    assert!(checked_steps > 0, "No feature steps were checked");
    println!("Checked {checked_steps} feature steps without executing them");
}
