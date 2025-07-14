use rustc_hash::FxHashSet;
use triomphe::Arc;
use std::collections::HashMap;

use crate::{Block, RValue, RcLocal, Statement, Traverse, Upvalue};

#[derive(Debug)]
struct NamingContext {
    method_name: String,
    argument: String,
    kind: String,
}

pub struct Namer {
    rename: bool,
    counter: usize,
    upvalues: FxHashSet<RcLocal>,
    naming_patterns: HashMap<String, Vec<String>>,
    context_stack: Vec<NamingContext>,
}

impl Namer {
    fn new(rename: bool) -> Self {
        let mut naming_patterns = HashMap::new();
        
        // Roblox-specific patterns without suffixes
        naming_patterns.insert("WaitForChild".to_string(), vec!["".to_string()]);
        naming_patterns.insert("FindFirstChild".to_string(), vec!["".to_string()]);
        naming_patterns.insert("GetService".to_string(), vec!["".to_string()]);
        naming_patterns.insert("Create".to_string(), vec!["".to_string()]);
        naming_patterns.insert("Clone".to_string(), vec!["Clone".to_string()]);
        naming_patterns.insert("new".to_string(), vec!["".to_string()]);

        Self {
            rename,
            counter: 1,
            upvalues: FxHashSet::default(),
            naming_patterns,
            context_stack: Vec::new(), // ✅ This is correct
        }
    }

    fn generate_meaningful_name(&self, value: &RValue) -> Option<String> {
        match value {
            RValue::Call(call) => {
                // Get method name, handling UTF-8 conversion as per your fixes
                if let Some(method_name) = call.get_method_name() {
                    if let Some(arg) = call.get_first_argument() {
                        // Clean the argument (now properly handling UTF-8 strings)
                        let clean_arg = arg.trim_matches(|c| c == '\'' || c == '"')
                            .replace(" ", "");
                        
                        // Get suffix (if any)
                        let suffix = self.naming_patterns
                            .get(&method_name)
                            .map_or("".to_string(), |v| v.join(""));
                        
                        if !clean_arg.is_empty() {
                            return Some(format!("{}{}", clean_arg, suffix));
                        }
                    }
                }
            }
            RValue::Index(index) => {
                // Try to generate name from index access
                if let Some(key_name) = index.get_key_name() {
                    return Some(key_name);
                }
            }
            _ => {}
        }
        None
    }

    fn name_local(&mut self, prefix: &str, local: &RcLocal, value: Option<&RValue>) {
        let mut lock = local.0.0.lock();
        
        if self.rename || lock.0.is_none() {
            // Single reference locals get underscore
            if Arc::count(&local.0.0) == 1 {
                lock.0 = Some("_".to_string());
                return;
            }

            // Try meaningful name first
            if let Some(value) = value {
                if let Some(meaningful_name) = self.generate_meaningful_name(value) {
                    lock.0 = Some(meaningful_name);
                    return;
                }
            }

            // Better fallback names
            let base_name = match prefix {
                "param" => "arg",
                "iter" => "iter",
                "index" => "i",
                _ => "var"
            };

            // Generate numbered name
            let number = if self.counter == 1 { 
                "".to_string() 
            } else { 
                self.counter.to_string() 
            };
            
            lock.0 = Some(format!("{}_{}", base_name, number));
            self.counter += 1;
        }
    }

    fn name_locals(&mut self, block: &mut Block) {
        for statement in &mut block.0 {
            statement.post_traverse_values(&mut |value| -> Option<()> {
                if let itertools::Either::Right(RValue::Closure(closure)) = value {
                    let mut function = closure.function.lock();
                    
                    // Name parameters
                    for param in &function.parameters {
                        self.name_local("param", param, None);
                    }
                    
                    self.name_locals(&mut function.body);
                };
                None
            });

            match statement {
                Statement::Assign(assign) if assign.prefix => {
                    for (idx, lvalue) in assign.left.iter().enumerate() {
                        if let Some(local) = lvalue.as_local() {
                            let value = assign.right.get(idx);
                            self.name_local("var", local, value);
                        }
                    }
                }
                Statement::If(r#if) => {
                    self.name_locals(&mut r#if.then_block.lock());
                    self.name_locals(&mut r#if.else_block.lock());
                }
                Statement::While(r#while) => {
                    self.name_locals(&mut r#while.block.lock());
                }
                Statement::Repeat(repeat) => {
                    self.name_locals(&mut repeat.block.lock());
                }
                Statement::NumericFor(numeric_for) => {
                    self.name_local("index", &numeric_for.counter, None);
                    self.name_locals(&mut numeric_for.block.lock());
                }
                Statement::GenericFor(generic_for) => {
                    for res_local in &generic_for.res_locals {
                        self.name_local("iter", res_local, None);
                    }
                    self.name_locals(&mut generic_for.block.lock());
                }
                _ => {}
            }
        }
    }

    fn find_upvalues(&mut self, block: &mut Block) {
        for statement in &mut block.0 {
            statement.post_traverse_values(&mut |value| -> Option<()> {
                if let itertools::Either::Right(RValue::Closure(closure)) = value {
                    self.upvalues.extend(
                        closure
                            .upvalues
                            .iter()
                            .map(|u| match u {
                                Upvalue::Copy(l) | Upvalue::Ref(l) => l,
                            })
                            .cloned(),
                    );
                    self.find_upvalues(&mut closure.function.lock().body);
                };
                None
            });

            match statement {
                Statement::If(r#if) => {
                    self.find_upvalues(&mut r#if.then_block.lock());
                    self.find_upvalues(&mut r#if.else_block.lock());
                }
                Statement::While(r#while) => {
                    self.find_upvalues(&mut r#while.block.lock());
                }
                Statement::Repeat(repeat) => {
                    self.find_upvalues(&mut repeat.block.lock());
                }
                Statement::NumericFor(numeric_for) => {
                    self.find_upvalues(&mut numeric_for.block.lock());
                }
                Statement::GenericFor(generic_for) => {
                    self.find_upvalues(&mut generic_for.block.lock());
                }
                _ => {}
            }
        }
    }
}

// Public interface
pub fn name_locals(block: &mut Block, rename: bool) {
    let mut namer = Namer::new(rename);
    namer.find_upvalues(block);
    namer.name_locals(block);
}