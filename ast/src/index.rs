use crate::{formatter::Formatter, has_side_effects, LocalRw, RcLocal, Traverse};

use super::RValue;
use std::fmt;

use crate::Literal;
use std::ops::Deref;

#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    pub left: Box<RValue>,
    pub right: Box<RValue>,
}

// this should be the same as MethodCall
has_side_effects!(Index);

impl Index {
    pub fn new(left: RValue, right: RValue) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn get_base_name(&self) -> Option<String> {
        match self.left.deref() {
            RValue::Local(local) => local.0.0.lock().0.clone(),
            RValue::Global(global) => {
                String::from_utf8(global.0.clone()).ok()
            },
            _ => None
        }
    }

    pub fn get_key_name(&self) -> Option<String> {
        match self.right.deref() {
            RValue::Literal(Literal::String(s)) => {
                String::from_utf8(s.clone()).ok()
            },
            RValue::Literal(Literal::Number(n)) => Some(n.to_string()),
            _ => None,
        }
    }
}

impl LocalRw for Index {
    fn values_read(&self) -> Vec<&RcLocal> {
        self.left
            .values_read()
            .into_iter()
            .chain(self.right.values_read().into_iter())
            .collect()
    }

    fn values_read_mut(&mut self) -> Vec<&mut RcLocal> {
        self.left
            .values_read_mut()
            .into_iter()
            .chain(self.right.values_read_mut().into_iter())
            .collect()
    }
}

impl Traverse for Index {
    fn rvalues_mut(&mut self) -> Vec<&mut RValue> {
        vec![&mut self.left, &mut self.right]
    }

    fn rvalues(&self) -> Vec<&RValue> {
        vec![&self.left, &self.right]
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Formatter {
            indentation_level: 0,
            indentation_mode: Default::default(),
            output: f,
        }
        .format_index(self)
    }
}
