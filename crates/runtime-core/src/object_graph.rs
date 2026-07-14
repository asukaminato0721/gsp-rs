use crate::dependency::{DependencyNodeInput, DependencyPlan, DependencyPlanError};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ObjectDefinition<Op> {
    Source,
    Derived { op: Op, parents: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ObjectNode<Op> {
    pub id: String,
    pub definition: ObjectDefinition<Op>,
}

impl<Op> ObjectNode<Op> {
    pub fn source(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            definition: ObjectDefinition::Source,
        }
    }

    pub fn derived(
        id: impl Into<String>,
        op: Op,
        parents: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            id: id.into(),
            definition: ObjectDefinition::Derived {
                op,
                parents: parents.into_iter().map(Into::into).collect(),
            },
        }
    }

    pub fn parents(&self) -> &[String] {
        match &self.definition {
            ObjectDefinition::Source => &[],
            ObjectDefinition::Derived { parents, .. } => parents,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectGraph<Op> {
    nodes: Vec<ObjectNode<Op>>,
    plan: DependencyPlan,
}

impl<Op> ObjectGraph<Op> {
    pub fn build(nodes: Vec<ObjectNode<Op>>) -> Result<Self, DependencyPlanError> {
        let dependency_nodes = nodes
            .iter()
            .map(|node| DependencyNodeInput {
                id: node.id.clone(),
                depends_on: node.parents().to_vec(),
            })
            .collect::<Vec<_>>();
        let plan = DependencyPlan::build_strict(&dependency_nodes)?;
        Ok(Self { nodes, plan })
    }

    pub fn nodes(&self) -> &[ObjectNode<Op>] {
        &self.nodes
    }

    pub fn node(&self, id: &str) -> Option<&ObjectNode<Op>> {
        self.plan
            .node_index(id)
            .and_then(|index| self.nodes.get(index))
    }

    pub fn topo_order(&self) -> &[usize] {
        self.plan.topo_order()
    }

    pub fn affected(&self, dirty_roots: &[String]) -> Vec<usize> {
        self.plan.affected(dirty_roots)
    }
}

pub trait OperationTable<Op, Value> {
    type Error;

    fn evaluate(
        &mut self,
        node_id: &str,
        op: &Op,
        parents: &[&Value],
    ) -> Result<Value, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectEvaluationError<Error> {
    UnknownNode(String),
    NotSource(String),
    MissingValue { node: String, parent: String },
    Operation { node: String, source: Error },
}

impl<Error: std::fmt::Display> std::fmt::Display for ObjectEvaluationError<Error> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownNode(id) => write!(formatter, "unknown object graph node: {id}"),
            Self::NotSource(id) => write!(formatter, "object graph node is not a source: {id}"),
            Self::MissingValue { node, parent } => {
                write!(
                    formatter,
                    "object graph node {node} has no value for parent {parent}"
                )
            }
            Self::Operation { node, source } => {
                write!(
                    formatter,
                    "object graph operation failed at {node}: {source}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectValues<Value> {
    values: Vec<Option<Value>>,
}

impl<Value> ObjectValues<Value> {
    pub fn new<Op>(graph: &ObjectGraph<Op>) -> Self {
        Self {
            values: (0..graph.nodes.len()).map(|_| None).collect(),
        }
    }

    pub fn set_source<Op, Error>(
        &mut self,
        graph: &ObjectGraph<Op>,
        id: &str,
        value: Value,
    ) -> Result<(), ObjectEvaluationError<Error>> {
        let Some(index) = graph.plan.node_index(id) else {
            return Err(ObjectEvaluationError::UnknownNode(id.into()));
        };
        if !matches!(graph.nodes[index].definition, ObjectDefinition::Source) {
            return Err(ObjectEvaluationError::NotSource(id.into()));
        }
        self.values[index] = Some(value);
        Ok(())
    }

    pub fn get<Op>(&self, graph: &ObjectGraph<Op>, id: &str) -> Option<&Value> {
        graph
            .plan
            .node_index(id)
            .and_then(|index| self.values.get(index))
            .and_then(Option::as_ref)
    }

    pub fn evaluate_all<Op, Table>(
        &mut self,
        graph: &ObjectGraph<Op>,
        table: &mut Table,
    ) -> Result<(), ObjectEvaluationError<Table::Error>>
    where
        Table: OperationTable<Op, Value>,
    {
        self.evaluate_indices(graph, graph.topo_order(), table)
    }

    pub fn evaluate_affected<Op, Table>(
        &mut self,
        graph: &ObjectGraph<Op>,
        dirty_roots: &[String],
        table: &mut Table,
    ) -> Result<(), ObjectEvaluationError<Table::Error>>
    where
        Table: OperationTable<Op, Value>,
    {
        let affected = graph.affected(dirty_roots);
        self.evaluate_indices(graph, &affected, table)
    }

    fn evaluate_indices<Op, Table>(
        &mut self,
        graph: &ObjectGraph<Op>,
        indices: &[usize],
        table: &mut Table,
    ) -> Result<(), ObjectEvaluationError<Table::Error>>
    where
        Table: OperationTable<Op, Value>,
    {
        for &index in indices {
            let node = &graph.nodes[index];
            let ObjectDefinition::Derived { op, parents } = &node.definition else {
                continue;
            };
            let parent_values = parents
                .iter()
                .map(|parent| {
                    let parent_index = graph
                        .plan
                        .node_index(parent)
                        .expect("strict graph validation guarantees every parent exists");
                    self.values[parent_index].as_ref().ok_or_else(|| {
                        ObjectEvaluationError::MissingValue {
                            node: node.id.clone(),
                            parent: parent.clone(),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = table
                .evaluate(&node.id, op, &parent_values)
                .map_err(|source| ObjectEvaluationError::Operation {
                    node: node.id.clone(),
                    source,
                })?;
            self.values[index] = Some(value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ArithmeticOp {
        Add,
        Double,
    }

    struct ArithmeticTable;

    impl OperationTable<ArithmeticOp, i64> for ArithmeticTable {
        type Error = &'static str;

        fn evaluate(
            &mut self,
            _node_id: &str,
            op: &ArithmeticOp,
            parents: &[&i64],
        ) -> Result<i64, Self::Error> {
            match op {
                ArithmeticOp::Add if parents.len() == 2 => Ok(*parents[0] + *parents[1]),
                ArithmeticOp::Double if parents.len() == 1 => Ok(*parents[0] * 2),
                _ => Err("wrong arity"),
            }
        }
    }

    fn arithmetic_graph() -> ObjectGraph<ArithmeticOp> {
        ObjectGraph::build(vec![
            ObjectNode::source("a"),
            ObjectNode::source("b"),
            ObjectNode::derived("sum", ArithmeticOp::Add, ["a", "b"]),
            ObjectNode::derived("result", ArithmeticOp::Double, ["sum"]),
        ])
        .unwrap()
    }

    #[test]
    fn evaluates_derived_nodes_from_source_values() {
        let graph = arithmetic_graph();
        let mut values = ObjectValues::new(&graph);
        values.set_source::<_, &str>(&graph, "a", 3).unwrap();
        values.set_source::<_, &str>(&graph, "b", 4).unwrap();
        values.evaluate_all(&graph, &mut ArithmeticTable).unwrap();
        assert_eq!(values.get(&graph, "sum"), Some(&7));
        assert_eq!(values.get(&graph, "result"), Some(&14));
    }

    #[test]
    fn incrementally_recomputes_only_affected_operations() {
        struct CountingTable(Vec<String>);
        impl OperationTable<ArithmeticOp, i64> for CountingTable {
            type Error = &'static str;

            fn evaluate(
                &mut self,
                node_id: &str,
                op: &ArithmeticOp,
                parents: &[&i64],
            ) -> Result<i64, Self::Error> {
                self.0.push(node_id.into());
                ArithmeticTable.evaluate(node_id, op, parents)
            }
        }

        let graph = arithmetic_graph();
        let mut values = ObjectValues::new(&graph);
        values.set_source::<_, &str>(&graph, "a", 3).unwrap();
        values.set_source::<_, &str>(&graph, "b", 4).unwrap();
        values.evaluate_all(&graph, &mut ArithmeticTable).unwrap();
        values.set_source::<_, &str>(&graph, "a", 5).unwrap();
        let mut table = CountingTable(Vec::new());
        values
            .evaluate_affected(&graph, &["a".into()], &mut table)
            .unwrap();
        assert_eq!(table.0, ["sum", "result"]);
        assert_eq!(values.get(&graph, "result"), Some(&18));
    }

    #[test]
    fn rejects_missing_parents_and_cycles_before_evaluation() {
        assert_eq!(
            ObjectGraph::build(vec![
                ObjectNode::source("a"),
                ObjectNode::derived("b", ArithmeticOp::Double, ["missing"]),
            ]),
            Err(DependencyPlanError::MissingDependency {
                node: "b".into(),
                dependency: "missing".into(),
            })
        );
        assert!(matches!(
            ObjectGraph::build(vec![
                ObjectNode::derived("a", ArithmeticOp::Double, ["b"]),
                ObjectNode::derived("b", ArithmeticOp::Double, ["a"]),
            ]),
            Err(DependencyPlanError::Cycle(_))
        ));
    }
}
