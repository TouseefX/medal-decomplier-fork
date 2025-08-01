use crate::{formatter, LocalRw, RValue, RcLocal, SideEffects, Traverse};

#[derive(Debug, Clone, PartialEq)]
pub struct SetList {
    pub object_local: RcLocal,
    pub index: usize,
    pub values: Vec<RValue>,
    pub tail: Option<RValue>,
}

impl SetList {
    pub fn new(
        object_local: RcLocal,
        index: usize,
        values: Vec<RValue>,
        tail: Option<RValue>,
    ) -> Self {
        Self {
            object_local,
            index,
            values,
            tail,
        }
    }
}

impl LocalRw for SetList {
    fn values_read(&self) -> Vec<&RcLocal> {
        let tail_locals = self
            .tail
            .as_ref()
            .map(|t| t.values_read())
            .unwrap_or_default();
        std::iter::once(&self.object_local)
            .chain(self.values.iter().flat_map(|rvalue| rvalue.values_read()))
            .chain(tail_locals)
            .collect()
    }

    fn values_read_mut(&mut self) -> Vec<&mut RcLocal> {
        let tail_locals = self
            .tail
            .as_mut()
            .map(|t| t.values_read_mut())
            .unwrap_or_default();
        std::iter::once(&mut self.object_local)
            .chain(
                self.values
                    .iter_mut()
                    .flat_map(|rvalue| rvalue.values_read_mut()),
            )
            .chain(tail_locals)
            .collect()
    }
}

impl SideEffects for SetList {
    fn has_side_effects(&self) -> bool {
        self.values
            .iter()
            .chain(self.tail.as_ref())
            .any(|rvalue| rvalue.has_side_effects())
    }
}

impl Traverse for SetList {
    fn rvalues(&self) -> Vec<&RValue> {
        self.values.iter().chain(self.tail.as_ref()).collect()
    }

    fn rvalues_mut(&mut self) -> Vec<&mut RValue> {
        self.values.iter_mut().chain(self.tail.as_mut()).collect()
    }
}

impl std::fmt::Display for SetList {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let all_tables = self.tail.is_none() && self.values.iter().all(|v| matches!(v, crate::RValue::Table(_)));
        let has_non_table = self.values.iter().any(|v| !matches!(v, crate::RValue::Table(_)));
        if all_tables {
            write!(f, "local {} = {{", self.object_local)?;
            for (i, value) in self.values.iter().enumerate() {
                if i != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", value)?;
            }
            write!(f, "}}")
        } else {
            write!(
                f,
                "local {} = __set_list({}, {})",
                self.object_local,
                self.index,
                formatter::format_arg_list(
                    &self
                        .values
                        .iter()
                        .chain(self.tail.as_ref())
                        .cloned()
                        .collect::<Vec<_>>()
                )
            )?;
            write!(
                f,
                "{}",
                if has_non_table { " -- WARNING: non-table value in set_list!" } else { " -- set the table your self" }
            )
        }
    }
}
