use super::{Literal, RValue};

/// Your global values. Update as needed based on your actual `Global` definition.
#[derive(Debug)]
pub enum Global {
    Game,
    Script,
    Workspace,
    Players,
    ReplicatedStorage,
    StarterGui,
    StarterPlayer,
    TweenService,
    Debris,
    // Add others if needed
}

pub trait NameGenerator {
    fn generate_name(&self, rvalue: &RValue, identifier: usize) -> Option<String>;
}

pub struct DefaultNameGenerator {}

impl NameGenerator for DefaultNameGenerator {
    fn generate_name(&self, rvalue: &RValue, identifier: usize) -> Option<String> {
        let hint = match rvalue {
            // Global values like `game`, `script`
            RValue::Global(global) => {
                let name = match global {
                    Global::Game => "game",
                    Global::Script => "script",
                    Global::Workspace => "workspace",
                    Global::Players => "players",
                    Global::ReplicatedStorage => "replicatedStorage",
                    Global::StarterGui => "starterGui",
                    Global::StarterPlayer => "starterPlayer",
                    Global::TweenService => "tweenService",
                    Global::Debris => "debris",
                    // Add other variants or return None
                    _ => return None,
                };
                Some(name.to_string())
            }

            // Index expressions like game["RunService"]
            RValue::Index(index) => match &*index.right {
                RValue::Literal(Literal::String(string)) => Some(string.clone()),
                _ => None,
            },

            // Method calls like game:GetService("RunService") or parent:WaitForChild("Gui")
            RValue::Call { function, args } => {
                if let RValue::Field { field, .. } = &**function {
                    match field.as_str() {
                        "GetService" | "WaitForChild" | "FindFirstChild" => {
                            if let Some(RValue::Literal(Literal::String(name))) = args.get(0) {
                                Some(name.clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }

            _ => None,
        };

        hint.map(|hint| format!("{}_{}", sanitize_name(&hint), identifier))
    }
}

/// Replaces non-alphanumeric characters and avoids identifiers starting with digits
fn sanitize_name(name: &str) -> String {
    let mut result = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    if result.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        result = format!("_{}", result);
    }

    result
}
