use super::*;

impl NodeGraph {
    /// Case-insensitive global search over node labels and content.
    pub fn search_nodes(&self, query: &str) -> Vec<u32> {
        self.authoring_graph().search_nodes(query)
    }
}
