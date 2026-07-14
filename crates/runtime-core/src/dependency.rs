use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyNodeInput {
    pub id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyPlan {
    node_ids: Vec<String>,
    node_indices: BTreeMap<String, usize>,
    topo_order: Vec<usize>,
    reverse_edges: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyPlanError {
    DuplicateNode(String),
    MissingDependency { node: String, dependency: String },
    Cycle(Vec<String>),
}

impl std::fmt::Display for DependencyPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(formatter, "duplicate scene dependency node: {id}"),
            Self::MissingDependency { node, dependency } => write!(
                formatter,
                "scene dependency node {node} references missing dependency {dependency}"
            ),
            Self::Cycle(nodes) => write!(
                formatter,
                "cyclic scene dependency graph: {}",
                nodes.join(", ")
            ),
        }
    }
}

impl DependencyPlan {
    pub fn build(nodes: &[DependencyNodeInput]) -> Result<Self, DependencyPlanError> {
        Self::build_with_missing_policy(nodes, false)
    }

    pub fn build_strict(nodes: &[DependencyNodeInput]) -> Result<Self, DependencyPlanError> {
        Self::build_with_missing_policy(nodes, true)
    }

    fn build_with_missing_policy(
        nodes: &[DependencyNodeInput],
        reject_missing_dependencies: bool,
    ) -> Result<Self, DependencyPlanError> {
        let mut node_indices = BTreeMap::new();
        for (index, node) in nodes.iter().enumerate() {
            if node_indices.insert(node.id.clone(), index).is_some() {
                return Err(DependencyPlanError::DuplicateNode(node.id.clone()));
            }
        }

        let mut indegrees = vec![0_usize; nodes.len()];
        let mut reverse_edges = vec![Vec::new(); nodes.len()];
        for (index, node) in nodes.iter().enumerate() {
            let mut seen = BTreeSet::new();
            for dependency in &node.depends_on {
                if !seen.insert(dependency) {
                    continue;
                }
                let dependency_index = if dependency == &node.id {
                    if !reject_missing_dependencies {
                        continue;
                    }
                    index
                } else if let Some(&dependency_index) = node_indices.get(dependency) {
                    dependency_index
                } else if reject_missing_dependencies {
                    return Err(DependencyPlanError::MissingDependency {
                        node: node.id.clone(),
                        dependency: dependency.clone(),
                    });
                } else {
                    continue;
                };
                indegrees[index] += 1;
                reverse_edges[dependency_index].push(index);
            }
        }

        let mut queue = indegrees
            .iter()
            .enumerate()
            .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
            .collect::<VecDeque<_>>();
        let mut topo_order = Vec::with_capacity(nodes.len());
        while let Some(index) = queue.pop_front() {
            topo_order.push(index);
            for &dependent_index in &reverse_edges[index] {
                indegrees[dependent_index] -= 1;
                if indegrees[dependent_index] == 0 {
                    queue.push_back(dependent_index);
                }
            }
        }
        if topo_order.len() != nodes.len() {
            return Err(DependencyPlanError::Cycle(
                nodes
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| indegrees[*index] > 0)
                    .map(|(_, node)| node.id.clone())
                    .collect(),
            ));
        }

        Ok(Self {
            node_ids: nodes.iter().map(|node| node.id.clone()).collect(),
            node_indices,
            topo_order,
            reverse_edges,
        })
    }

    pub fn topo_order(&self) -> &[usize] {
        &self.topo_order
    }

    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.node_indices.get(id).copied()
    }

    pub fn affected(&self, dirty_roots: &[String]) -> Vec<usize> {
        let mut affected = vec![false; self.node_ids.len()];
        let mut queue = VecDeque::new();
        for root in dirty_roots {
            let Some(&index) = self.node_indices.get(root) else {
                continue;
            };
            if !affected[index] {
                affected[index] = true;
                queue.push_back(index);
            }
        }
        while let Some(index) = queue.pop_front() {
            for &dependent_index in &self.reverse_edges[index] {
                if !affected[dependent_index] {
                    affected[dependent_index] = true;
                    queue.push_back(dependent_index);
                }
            }
        }
        self.topo_order
            .iter()
            .copied()
            .filter(|index| affected[*index])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, depends_on: &[&str]) -> DependencyNodeInput {
        DependencyNodeInput {
            id: id.into(),
            depends_on: depends_on.iter().map(|value| (*value).into()).collect(),
        }
    }

    #[test]
    fn plan_orders_and_filters_affected_nodes() {
        let plan = DependencyPlan::build(&[
            node("root-a", &[]),
            node("root-b", &[]),
            node("derived-a", &["root-a"]),
            node("combined", &["derived-a", "root-b"]),
        ])
        .unwrap();
        assert_eq!(plan.topo_order(), &[0, 1, 2, 3]);
        assert_eq!(plan.affected(&["root-a".into()]), vec![0, 2, 3]);
        assert_eq!(plan.affected(&["missing".into()]), Vec::<usize>::new());
    }

    #[test]
    fn plan_rejects_cycles_and_duplicate_ids() {
        assert_eq!(
            DependencyPlan::build(&[node("a", &["b"]), node("b", &["a"])]),
            Err(DependencyPlanError::Cycle(vec!["a".into(), "b".into()])),
        );
        assert_eq!(
            DependencyPlan::build(&[node("a", &[]), node("a", &[])]),
            Err(DependencyPlanError::DuplicateNode("a".into())),
        );
        assert_eq!(
            DependencyPlan::build_strict(&[node("a", &["missing"])]),
            Err(DependencyPlanError::MissingDependency {
                node: "a".into(),
                dependency: "missing".into(),
            }),
        );
    }
}
