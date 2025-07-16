use std::collections::HashMap;

use itertools::Either;

use crate::{Block, LocalRw, RValue, RcLocal, Statement, Traverse};

pub fn replace_locals<H: std::hash::BuildHasher>(
    block: &mut Block,
    map: &HashMap<RcLocal, RcLocal, H>,
) {
    for statement in &mut block.0 {
        for local in statement.values_read_mut() {
            if let Some(new_local) = map.get(local) {
                *local = new_local.clone();
            }
        }
        for local in statement.values_written_mut() {
            if let Some(new_local) = map.get(local) {
                *local = new_local.clone();
            }
        }
        // TODO: traverse_values
        statement.post_traverse_values(&mut |value| -> Option<()> {
            if let Either::Right(RValue::Closure(closure)) = value {
                replace_locals(&mut closure.function.lock().body, map)
            };
            None
        });
        match statement {
            Statement::If(r#if) => {
                replace_locals(&mut r#if.then_block.lock(), map);
                replace_locals(&mut r#if.else_block.lock(), map);
            }
            Statement::While(r#while) => {
                replace_locals(&mut r#while.block.lock(), map);
            }
            Statement::Repeat(repeat) => {
                replace_locals(&mut repeat.block.lock(), map);
            }
            Statement::NumericFor(numeric_for) => {
                replace_locals(&mut numeric_for.block.lock(), map);
            }
            Statement::GenericFor(generic_for) => {
                replace_locals(&mut generic_for.block.lock(), map);
            }
            _ => {}
        }
    }
}

/// Panics if any goto or label is found in the AST block (for enforcing structured output).
pub fn fail_on_goto(block: &crate::Block) {
    for stmt in &block.0 {
        match stmt {
            crate::Statement::Goto(_) | crate::Statement::Label(_) => {
                panic!("Unstructured control flow (goto/label) found in output! Please improve structuring.");
            }
            crate::Statement::If(r#if) => {
                fail_on_goto(&r#if.then_block.lock());
                fail_on_goto(&r#if.else_block.lock());
            }
            crate::Statement::While(r#while) => {
                fail_on_goto(&r#while.block.lock());
            }
            crate::Statement::Repeat(repeat) => {
                fail_on_goto(&repeat.block.lock());
            }
            crate::Statement::NumericFor(numeric_for) => {
                fail_on_goto(&numeric_for.block.lock());
            }
            crate::Statement::GenericFor(generic_for) => {
                fail_on_goto(&generic_for.block.lock());
            }
            _ => {}
        }
    }
}
