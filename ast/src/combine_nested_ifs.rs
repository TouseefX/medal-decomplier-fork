use crate::{Block, Statement, RValue, LValue};
use crate::traverse::Traverse;
use crate::local::LocalRw;

fn replace_local_in_rvalue(rvalue: &mut RValue, target: &crate::local::RcLocal, replacement: &RValue) {
    if let RValue::Local(l) = rvalue {
        if l == target {
            *rvalue = replacement.clone();
        }
    } else {
        for child in rvalue.rvalues_mut() {
            replace_local_in_rvalue(child, target, replacement);
        }
    }
}

/// Combines nested ifs where a local variable is only used in the inner if condition.
pub fn combine_nested_ifs(block: &mut Block) {
    let mut i = 0;
    while i < block.len() {
        // Look for: local x = expr; if x then ... end
        if let Statement::Assign(assign) = &block[i] {
            if assign.left.len() == 1 && assign.right.len() == 1 {
                if let LValue::Local(local) = &assign.left[0] {
                    if i + 1 < block.len() {
                        if let Statement::If(inner_if) = &block[i + 1] {
                            // Check if the condition is just the local
                            let cond_is_local = match &inner_if.condition {
                                RValue::Local(l) => l == local,
                                _ => false,
                            };
                            // Check if the local is used in the then-block
                            let used_in_then = inner_if.then_block.lock().iter()
                                .flat_map(|stmt| stmt.values_read())
                                .any(|l| l == local);
                            // Check if the local is used after the if
                            let used_after = block[i + 2..].iter()
                                .flat_map(|stmt| stmt.values_read())
                                .any(|l| l == local);
                            if cond_is_local && !used_after {
                                // Instead of using the local variable in the condition, use the assigned expression directly
                                let mut new_if = inner_if.clone();
                                new_if.condition = assign.right[0].clone();
                                if used_in_then {
                                    // Move the assignment inside the then-block
                                    let mut then_block = new_if.then_block.lock();
                                    then_block.0.insert(0, Statement::Assign(assign.clone()));
                                }
                                // Remove the assign and replace the if
                                block.remove(i);
                                block[i] = Statement::If(new_if);
                                continue;
                            }
                        }
                    }
                }
            }
        }
        // Recurse into blocks
        match &mut block[i] {
            Statement::If(if_stmt) => {
                combine_nested_ifs(&mut if_stmt.then_block.lock());
                combine_nested_ifs(&mut if_stmt.else_block.lock());
            }
            Statement::While(while_stmt) => {
                combine_nested_ifs(&mut while_stmt.block.lock());
            }
            Statement::Repeat(repeat_stmt) => {
                combine_nested_ifs(&mut repeat_stmt.block.lock());
            }
            Statement::NumericFor(nf) => {
                combine_nested_ifs(&mut nf.block.lock());
            }
            Statement::GenericFor(gf) => {
                combine_nested_ifs(&mut gf.block.lock());
            }
            _ => {}
        }
        i += 1;
    }
} 
