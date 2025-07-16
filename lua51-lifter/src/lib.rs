mod lifter;
use lifter::Lifter;
use lua51_deserializer::chunk::Chunk;
use triomphe::Arc;
use parking_lot::Mutex;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use petgraph::algo::dominators::simple_fast;
use cfg::ssa::structuring::{structure_jumps, structure_conditionals};
use ast::local_declarations::LocalDeclarer;
use by_address::ByAddress;
use ast::name_locals::name_locals;
use ast::replace_locals::fail_on_goto;

pub fn decompile_bytecode(bytecode: &[u8]) -> String {
    let chunk = Chunk::parse(bytecode).unwrap().1;
    let mut lifted = Vec::new();
    let (function, upvalues) = Lifter::lift(&chunk.function, &mut lifted);
    lifted.push((Arc::<Mutex<_>>::default(), function, upvalues));
    lifted.reverse();

    let (main, ..): (Arc<Mutex<ast::Function>>, cfg::function::Function, Vec<ast::RcLocal>) = lifted.first().unwrap().clone();
    let mut upvalues = lifted
        .into_iter()
        .map(|(ast_function, mut function, upvalues_in)| {
            let (local_count, local_groups, upvalue_in_groups, upvalue_passed_groups) =
                cfg::ssa::construct(&mut function, &upvalues_in);
            let upvalue_to_group = upvalue_in_groups
                .into_iter()
                .chain(
                    upvalue_passed_groups
                        .into_iter()
                        .map(|m| (ast::RcLocal::default(), m)),
                )
                .flat_map(|(i, g)| g.into_iter().map(move |u| (u, i.clone())))
                .collect::<IndexMap<_, _>>();
            let local_to_group = local_groups
                .into_iter()
                .enumerate()
                .flat_map(|(i, g)| g.into_iter().map(move |l| (l, i)))
                .collect::<FxHashMap<_, _>>();
            let mut changed = true;
            while changed {
                changed = false;
                let dominators = simple_fast(function.graph(), function.entry().unwrap());
                changed |= structure_jumps(&mut function, &dominators);
                cfg::ssa::inline::inline(&mut function, &local_to_group, &upvalue_to_group);
                if structure_conditionals(&mut function) {
                    changed = true;
                }
                let mut local_map = FxHashMap::default();
                if cfg::ssa::construct::remove_unnecessary_params(&mut function, &mut local_map) {
                    changed = true;
                }
                cfg::ssa::construct::apply_local_map(&mut function, local_map);
            }
            cfg::ssa::Destructor::new(
                &mut function,
                upvalue_to_group,
                upvalues_in.iter().cloned().collect(),
                local_count,
            )
            .destruct();
            let params = std::mem::take(&mut function.parameters);
            let is_variadic = function.is_variadic;
            let block = Arc::new(restructure::lift(function).into());
            LocalDeclarer::default().declare_locals(
                Arc::clone(&block),
                &upvalues_in.iter().chain(params.iter()).cloned().collect(),
            );
            {
                let mut ast_function = ast_function.lock();
                ast_function.body = Arc::try_unwrap(block).unwrap().into_inner();
                ast_function.parameters = params;
                ast_function.is_variadic = is_variadic;
            }
            (ByAddress(ast_function), upvalues_in)
        })
        .collect::<FxHashMap<_, _>>();
    let main = ByAddress(main);
    upvalues.remove(&main);
    let mut body = Arc::try_unwrap(main.0).unwrap().into_inner().body;
    fail_on_goto(&body);
    name_locals(&mut body, true);
    body.to_string()
}