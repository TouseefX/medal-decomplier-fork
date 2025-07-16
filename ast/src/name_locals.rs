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
    used_names: HashMap<String, usize>,
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
            used_names: HashMap::new(),
        }
    }

    fn unique_name(&mut self, base: &str) -> String {
        let count = self.used_names.entry(base.to_string()).or_insert(0);
        let name = if *count == 0 {
            base.to_string()
        } else {
            format!("{}_{}", base, count)
        };
        *count += 1;
        name
    }

    fn generate_meaningful_name(&self, value: &RValue) -> Option<String> {
        // Handle chained MethodCall/Call for WaitForChild/FindFirstChild/GetService
        let mut current = value;
        let mut last_name: Option<String> = None;
        loop {
            match current {
                RValue::Call(call) => {
                    if let Some(method_name) = call.get_method_name() {
                        if ["GetService", "WaitForChild", "FindFirstChild"].contains(&method_name.as_str()) {
                            if let Some(arg) = call.get_first_argument() {
                                let clean_arg = arg.trim_matches(|c| c == '\'' || c == '"').replace(" ", "");
                                let suffix = self.naming_patterns.get(&method_name).map_or("".to_string(), |v| v.join(""));
                                if !clean_arg.is_empty() {
                                    last_name = Some(format!("{}{}", clean_arg, suffix));
                                }
                            }
                        }
                    }
                    // For chaining, go deeper if the call's value is another call/methodcall
                    current = call.value.as_ref();
                }
                RValue::MethodCall(method_call) => {
                    let method_name = &method_call.method;
                    if ["GetService", "WaitForChild", "FindFirstChild"].contains(&method_name.as_str()) {
                        if let Some(arg) = method_call.arguments.get(0) {
                            match arg {
                                RValue::Literal(crate::Literal::String(s)) => {
                                    if let Ok(arg_str) = String::from_utf8(s.clone()) {
                                        let clean_arg = arg_str.trim_matches(|c| c == '\'' || c == '"').replace(" ", "");
                                        let suffix = self.naming_patterns.get(method_name).map_or("".to_string(), |v| v.join(""));
                                        if !clean_arg.is_empty() {
                                            last_name = Some(format!("{}{}", clean_arg, suffix));
                                        }
                                    }
                                }
                                RValue::Local(local) => {
                                    let base_name = match method_call.value.as_ref() {
                                        RValue::Index(index) => index.get_key_name(),
                                        RValue::Global(global) => String::from_utf8(global.0.clone()).ok(),
                                        _ => None,
                                    };
                                    if let Some(base) = base_name {
                                        let mut chars = base.chars();
                                        let capitalized = match chars.next() {
                                            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                            None => base,
                                        };
                                        last_name = Some(format!("Some{}", capitalized));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // For chaining, go deeper if the method_call's value is another call/methodcall
                    current = method_call.value.as_ref();
                }
                _ => break,
            }
        }
        if last_name.is_some() {
            return last_name;
        }
        // Handle require(...)
        if let RValue::Call(call) = value {
            if let Some(method_name) = call.get_method_name() {
                if method_name == "require" {
                    if let Some(arg) = call.arguments.get(0) {
                        let mut current = arg;
                        let mut last_key = None;
                        while let RValue::Index(index) = current {
                            if let Some(key) = index.get_key_name() {
                                last_key = Some(key);
                            }
                            current = &index.left;
                        }
                        if let Some(key) = last_key {
                            return Some(key);
                        }
                    }
                }
            }
        }
        // Handle GetService/WaitForChild/FindFirstChild
        if let RValue::Call(call) = value {
            if let Some(method_name) = call.get_method_name() {
                if ["GetService", "WaitForChild", "FindFirstChild"].contains(&method_name.as_str()) {
                    if let Some(arg) = call.get_first_argument() {
                        let clean_arg = arg.trim_matches(|c| c == '\'' || c == '"').replace(" ", "");
                        let suffix = self.naming_patterns.get(&method_name).map_or("".to_string(), |v| v.join(""));
                        if !clean_arg.is_empty() {
                            return Some(format!("{}{}", clean_arg, suffix));
                        }
                    }
                }
            }
        }
        if let RValue::MethodCall(method_call) = value {
            let method_name = &method_call.method;
            // Handle GetService/WaitForChild/FindFirstChild for MethodCall
            if ["GetService", "WaitForChild", "FindFirstChild"].contains(&method_name.as_str()) {
                if let Some(arg) = method_call.arguments.get(0) {
                    match arg {
                        RValue::Literal(crate::Literal::String(s)) => {
                            if let Ok(arg_str) = String::from_utf8(s.clone()) {
                                let clean_arg = arg_str.trim_matches(|c| c == '\'' || c == '"').replace(" ", "");
                                let suffix = self.naming_patterns.get(method_name).map_or("".to_string(), |v| v.join(""));
                                if !clean_arg.is_empty() {
                                    return Some(format!("{}{}", clean_arg, suffix));
                                }
                            }
                        }
                        RValue::Local(local) => {
                            let base_name = match method_call.value.as_ref() {
                                RValue::Index(index) => index.get_key_name(),
                                RValue::Global(global) => String::from_utf8(global.0.clone()).ok(),
                                _ => None,
                            };
                            if let Some(base) = base_name {
                                let mut chars = base.chars();
                                let capitalized = match chars.next() {
                                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                    None => base,
                                };
                                return Some(format!("Some{}", capitalized));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let RValue::Index(index) = value {
            if let Some(key_name) = index.get_key_name() {
                return Some(key_name);
            }
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
                    lock.0 = Some(self.unique_name(&meaningful_name));
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
            lock.0 = Some(self.unique_name(base_name));
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