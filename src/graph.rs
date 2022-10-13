use std::{collections::HashMap, iter, slice};

use anyhow::{anyhow, Context, Result};
use petgraph::graph::NodeIndex;
use petgraph::visit::DfsPostOrder;
use petgraph::Graph;

use crate::config::M2MTable;
use crate::sql::Relationship;

pub struct DepGraph<'a> {
    deps: Graph<&'a str, &'a str>,
    idx_by_name: HashMap<&'a str, NodeIndex>,
}

impl<'a> DepGraph<'a> {
    pub fn new<N, E>(nodes: N, edges: E) -> Result<DepGraph<'a>>
    where
        N: IntoIterator<Item = &'a str>,
        E: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut deps = Graph::<&str, _, _>::new();
        let idx_by_name = nodes
            .into_iter()
            .map(|table| (table, deps.add_node(table)))
            .collect::<HashMap<_, _>>();
        let edges: Vec<_> = edges
            .into_iter()
            .map(|relation| {
                Some((
                    *(idx_by_name).get(relation.0)?,
                    *(idx_by_name).get(relation.1)?,
                ))
            })
            .collect::<Option<Vec<_>>>()
            .with_context(|| "something went wrong building table graph")?;
        deps.extend_with_edges(edges);

        Ok(DepGraph { deps, idx_by_name })
    }

    pub fn get_dependencies_of(&self, node: &str) -> Result<Vec<String>> {
        let idx = self
            .idx_by_name
            .get(node)
            .ok_or_else(|| anyhow!("table {node} does not exist"))?;
        let mut tables = Vec::new();
        let mut dfs = DfsPostOrder::new(&self.deps, *idx);

        while let Some(node) = dfs.next(&self.deps) {
            tables.push(self.deps[node].into());
        }

        Ok(tables)
    }
}

pub fn tables_as_nodes(tables: &[String]) -> iter::Map<slice::Iter<String>, fn(&String) -> &str> {
    tables.iter().map(|table| table.as_str())
}

pub fn relationships_as_edges<'a>(
    relationships: &'a [Relationship],
    m2m_tables: &'a [M2MTable],
) -> impl Iterator<Item = (&'a str, &'a str)> + ExactSizeIterator + DoubleEndedIterator + 'a {
    relationships.iter().map(move |rel| {
        let dest_table = rel.dest_table.as_str();
        let source_table = rel.source_table.as_str();

        m2m_tables
            .iter()
            .find(|t| t.name == source_table)
            .filter(|m2m_table| m2m_table.source == rel.dest_table)
            .map(|_| (dest_table, source_table))
            .unwrap_or((source_table, dest_table))
    })
}
